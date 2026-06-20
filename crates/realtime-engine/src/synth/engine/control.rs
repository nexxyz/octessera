use super::*;

impl SynthEngine {
    pub fn momentary_fx_start(
        &mut self,
        id: String,
        fx_type: String,
        params: BTreeMap<String, Value>,
        target: MomentaryFxTarget,
    ) {
        let Some(kind) = parse_momentary_fx_kind(&fx_type) else {
            return;
        };
        self.momentary_fx.retain(|fx| fx.id != id);
        self.momentary_fx.push(MomentaryFxState::new(
            id,
            kind,
            params,
            target,
            self.sample_rate,
        ));

        if kind == MomentaryFxKind::PitchShift {
            if let Some(fx) = self.momentary_fx.last_mut() {
                let pos = self.dry_history_pos;
                let len = self.dry_history.len();
                let mut contiguous = Vec::with_capacity(len);
                if pos < len {
                    contiguous.extend_from_slice(&self.dry_history[pos..]);
                }
                if pos > 0 {
                    contiguous.extend_from_slice(&self.dry_history[..pos]);
                }
                fx.pitch_shifter.prefill(&contiguous);
            }
        }
    }

    pub fn momentary_fx_stop(&mut self, id: &str) {
        let should_remove = self
            .momentary_fx
            .iter()
            .find(|fx| fx.id == id)
            .map(|fx| {
                matches!(
                    fx.kind,
                    MomentaryFxKind::Stutter | MomentaryFxKind::PitchShift
                )
            })
            .unwrap_or(true);
        if should_remove {
            self.momentary_fx.retain(|fx| fx.id != id);
        } else if let Some(fx) = self.momentary_fx.iter_mut().find(|fx| fx.id == id) {
            fx.releasing = true;
            fx.release_pos = 0;
            if fx.kind == MomentaryFxKind::Freeze {
                let ms = param_f32(&fx.params, "releaseMs", 500.0);
                fx.release_len = ms_to_samples(ms, self.sample_rate).max(1);
            }
        }
    }

    pub fn momentary_fx_update(&mut self, id: &str, params: BTreeMap<String, Value>) {
        if let Some(fx) = self.momentary_fx.iter_mut().find(|fx| fx.id == id) {
            fx.params = params;
        }
    }

    pub fn set_voice_stealing_mode(&mut self, mode: VoiceStealingMode) {
        self.voice_stealing_mode = mode;
    }

    pub fn set_runtime_load_ratio(&mut self, ratio: f32) {
        let r = ratio.clamp(0.0, 2.0);
        self.smoothed_load_ratio = 0.9 * self.smoothed_load_ratio + 0.1 * r;
    }

    pub fn audio_load_status(&mut self) -> AudioLoadStatus {
        let status = AudioLoadStatus {
            ratio: self.smoothed_load_ratio,
            voice_steal: self.voice_steal_since_status,
        };
        self.voice_steal_since_status = false;
        status
    }

    pub fn set_instruments(&mut self, cfg: InstrumentsConfig) {
        self.pan_positions = cfg.pan_positions.max(1);
        self.master_volume = (cfg.master_volume / 100.0).clamp(0.0, 1.0);
        for (idx, slot) in cfg.instruments.into_iter().enumerate() {
            if idx >= INSTRUMENT_SLOT_COUNT {
                break;
            }
            self.slot_kind[idx] = parse_instrument_kind(&slot.kind);
            if self.slot_kind[idx] != InstrumentKind::Synth {
                if let Some(m) = slot.mixer {
                    self.slot_route[idx] = parse_route(&m.route);
                    self.slot_pan_pos[idx] = m.pan_pos.min(self.pan_positions - 1);
                    self.slot_volume[idx] = (m.volume / 100.0).clamp(0.0, 1.0);
                }
                continue;
            }
            self.instruments[idx] = slot.synth;
            if let Some(m) = slot.mixer {
                self.slot_route[idx] = parse_route(&m.route);
                self.slot_pan_pos[idx] = m.pan_pos.min(self.pan_positions - 1);
                self.slot_volume[idx] = (m.volume / 100.0).clamp(0.0, 1.0);
            }
        }
        let mut next_bus_pan_pos = Vec::new();
        let mut next_bus_slot_params = Vec::new();
        let mut next_bus_slot_state = Vec::new();
        let mut next_master_slot_params = Vec::new();
        let mut next_master_slot_state = Vec::new();
        if let Some(mixer) = cfg.mixer {
            for (bus_idx, bus) in mixer.buses.into_iter().enumerate() {
                next_bus_pan_pos.push(bus.pan_pos.min(self.pan_positions - 1));
                let mut cfgs: [FxBusSlotConfig; BUS_SLOTS_PER_BUS] =
                    std::array::from_fn(|_| FxBusSlotConfig::Kind("none".to_string()));
                for (j, slot) in bus.slots.into_iter().enumerate().take(BUS_SLOTS_PER_BUS) {
                    cfgs[j] = slot;
                }
                let params: [FxBusParams; BUS_SLOTS_PER_BUS] =
                    std::array::from_fn(|j| compile_fx_bus_params(&cfgs[j]));
                let states: [FxBusState; BUS_SLOTS_PER_BUS] = std::array::from_fn(|j| {
                    self.bus_slot_state
                        .get(bus_idx)
                        .and_then(|states| states.get(j))
                        .filter(|state| fx_bus_state_matches_params(state, &params[j]))
                        .cloned()
                        .unwrap_or_else(|| fx_bus_state_from_params(&params[j], self.sample_rate))
                });
                next_bus_slot_params.push(params);
                next_bus_slot_state.push(states);
            }
            if let Some(master) = mixer.master {
                for (slot_idx, slot) in master.slots.into_iter().enumerate() {
                    let params = compile_fx_bus_params(&slot);
                    let state = self
                        .master_slot_state
                        .get(slot_idx)
                        .filter(|state| master_fx_state_matches_params(state, &params))
                        .cloned()
                        .unwrap_or_else(|| master_fx_state_from_params(&params));
                    next_master_slot_params.push(params);
                    next_master_slot_state.push(state);
                }
            }
        }
        self.bus_pan_pos = next_bus_pan_pos;
        self.bus_slot_params = next_bus_slot_params;
        self.bus_slot_state = next_bus_slot_state;
        self.master_slot_params = next_master_slot_params;
        self.master_slot_state = next_master_slot_state;
        self.bus_mono_scratch.resize(self.bus_pan_pos.len(), 0.0);
    }

    pub fn set_sample_banks(&mut self, banks: Vec<SampleBankConfig>) {
        self.sample_banks = banks;
        self.sample_banks
            .resize(INSTRUMENT_SLOT_COUNT, SampleBankConfig::default());
        for pool in self.sample_voices.iter_mut() {
            for voice in pool.iter_mut() {
                voice.active = false;
            }
        }
    }

    pub fn preview_sample(&mut self, instrument_slot: u8, buffer: SampleBuffer, velocity: u8) {
        let slot = (instrument_slot as usize).min(INSTRUMENT_SLOT_COUNT - 1);
        if buffer.samples.is_empty() || buffer.channels == 0 || buffer.sample_rate == 0 {
            return;
        }
        let bank = self.sample_banks.get(slot).cloned().unwrap_or_default();
        let vel = (velocity.max(1) as f32 / 127.0).clamp(0.0, 1.0);
        let vel_sens = (bank.velocity_sensitivity_pct / 100.0).clamp(0.0, 1.0);
        let gain = (bank.gain_pct / 100.0).clamp(0.0, 2.0) * ((1.0 - vel_sens) + vel_sens * vel);
        let pitch = 2.0_f32.powf(bank.tune_semis / 12.0);
        let step = pitch * buffer.sample_rate as f32 / self.sample_rate as f32;
        self.preview_sample_voices.push(PreviewSampleVoice {
            slot,
            buffer,
            pos: 0.0,
            step,
            gain,
        });
    }

    pub fn note_on(&mut self, instrument_slot: u8, midi_note: u8, velocity: u8, duration_ms: u32) {
        let slot = (instrument_slot as usize).min(INSTRUMENT_SLOT_COUNT - 1);
        if self.slot_kind[slot] == InstrumentKind::Sample {
            self.sample_note_on(slot, midi_note, velocity);
            return;
        }
        if self.slot_kind[slot] != InstrumentKind::Synth {
            return;
        }
        let v = velocity.max(1);
        let duration_samples = ms_to_samples(duration_ms as f32, self.sample_rate).max(1) as u64;
        let note_off_sample = self.sample_clock.saturating_add(duration_samples);
        let freq = midi_note_to_hz(midi_note);

        let pool = &mut self.voices[slot];
        let voice_index = pool.iter().position(|voice| !voice.active);
        let i = match voice_index {
            Some(i) => i,
            None => {
                self.voice_steal_since_status = true;
                Self::steal_voice_index(pool)
            }
        };

        let cfg = self.instruments[slot];
        let amp_env = EnvState::note_on(cfg.amp_env, self.sample_rate);
        let filt_env = EnvState::note_on(cfg.filter_env, self.sample_rate);
        pool[i] = Voice {
            active: true,
            instrument_slot: slot as u8,
            midi_note,
            velocity: v,
            note_off_sample,
            started_sample: self.sample_clock,
            freq_hz: freq,
            phase1: 0.0,
            phase2: 0.0,
            amp_env,
            filt_env,
            filt: BiquadState::new(),
        };

        self.enforce_global_voice_budget();
    }

    pub fn cc(&mut self, instrument_slot: u8, controller: u8, value: u8) {
        let slot = (instrument_slot as usize).min(INSTRUMENT_SLOT_COUNT - 1);
        if self.slot_kind[slot] == InstrumentKind::None {
            return;
        }
        if controller == 74 {
            self.mods[slot].cutoff_cc = (value as f32 / 127.0).clamp(0.0, 1.0);
        } else if controller == 71 {
            self.mods[slot].resonance_cc = (value as f32 / 127.0).clamp(0.0, 1.0);
        } else if controller == 120 || controller == 123 {
            self.mods[slot] = InstrumentMod::new();
        }
    }

    pub fn note_off(&mut self, instrument_slot: u8, midi_note: u8) {
        let slot = (instrument_slot as usize).min(INSTRUMENT_SLOT_COUNT - 1);
        if self.slot_kind[slot] == InstrumentKind::None {
            return;
        }
        if self.slot_kind[slot] == InstrumentKind::Sample {
            let sample_slot = sample_slot_for_note(midi_note);
            for voice in self.sample_voices[slot].iter_mut() {
                if voice.active && voice.sample_slot == sample_slot {
                    voice.active = false;
                }
            }
            return;
        }
        let cfg = self.instruments[slot];
        for voice in self.voices[slot].iter_mut() {
            if !voice.active || voice.midi_note != midi_note {
                continue;
            }
            voice.amp_env.begin_release(cfg.amp_env, self.sample_rate);
            voice
                .filt_env
                .begin_release(cfg.filter_env, self.sample_rate);
            voice.note_off_sample = self.sample_clock;
        }
    }

    pub fn all_notes_off(&mut self) {
        self.preview_sample_voices.clear();
        for pool in self.sample_voices.iter_mut() {
            for voice in pool.iter_mut() {
                voice.active = false;
            }
        }
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            let cfg = self.instruments[slot];
            for voice in self.voices[slot].iter_mut() {
                if !voice.active {
                    continue;
                }
                voice.amp_env.begin_release(cfg.amp_env, self.sample_rate);
                voice
                    .filt_env
                    .begin_release(cfg.filter_env, self.sample_rate);
                voice.note_off_sample = self.sample_clock;
            }
        }
    }
}
