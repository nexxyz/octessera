use super::support::stutter_segment_len;
use super::*;

const MAX_MOMENTARY_FX: usize = 2;

struct CompiledBusMixerState {
    pan_positions: Vec<usize>,
    pan_gains: Vec<(f32, f32)>,
    slot_params: Vec<[FxBusParams; BUS_SLOTS_PER_BUS]>,
    slot_state: Vec<[FxBusState; BUS_SLOTS_PER_BUS]>,
    active_slot_indices: Vec<[usize; BUS_SLOTS_PER_BUS]>,
    active_slot_counts: Vec<usize>,
}

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
        if let Some(pos) = self.momentary_fx.iter().position(|fx| fx.id == id) {
            self.momentary_fx.remove(pos);
        }
        if self.momentary_fx.iter().any(|fx| fx.kind == kind) {
            return;
        }
        if self.momentary_fx.len() >= MAX_MOMENTARY_FX {
            return;
        }
        self.momentary_fx.push(MomentaryFxState::new(
            id,
            kind,
            params,
            target,
            self.sample_rate,
        ));

        if kind == MomentaryFxKind::PitchShift {
            let fx = self.momentary_fx.last_mut().expect("inserted momentary FX");
            fx.pitch_shifter
                .prefill_from_ring(&self.dry_history, self.dry_history_pos);
        }
    }

    pub fn momentary_fx_stop(&mut self, id: &str) {
        let Some(pos) = self.momentary_fx.iter().position(|fx| fx.id == id) else {
            return;
        };
        let should_remove = matches!(
            self.momentary_fx[pos].kind,
            MomentaryFxKind::Stutter | MomentaryFxKind::PitchShift
        );
        if should_remove {
            self.momentary_fx.remove(pos);
        } else {
            let fx = &mut self.momentary_fx[pos];
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
            if fx.kind == MomentaryFxKind::Stutter {
                fx.stutter_segment_len = stutter_segment_len(self.sample_rate, &fx.params);
                fx.stutter_write = 0;
                fx.stutter_ready = false;
                fx.stutter_ramp_pos = 0;
            }
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
            block_ratio_p95: 0.0,
            block_ratio_max: 0.0,
            blocks: 0,
            control_events: 0,
            config_events: 0,
        };
        self.voice_steal_since_status = false;
        status
    }

    pub fn set_instruments(&mut self, cfg: InstrumentsConfig) {
        self.pan_positions = cfg.pan_positions.max(1);
        self.master_volume = (cfg.master_volume / 100.0).clamp(0.0, 1.0);
        self.apply_instrument_slots_config(cfg.instruments);
        self.refresh_slot_pan_gains();
        let next_bus = self.compile_bus_mixer_state(cfg.mixer.as_ref());
        let (next_master_slot_params, next_master_slot_state) =
            self.compile_master_mixer_state(cfg.mixer.as_ref());
        self.bus_pan_pos = next_bus.pan_positions;
        self.bus_pan_gains_cache = next_bus.pan_gains;
        self.bus_slot_params = next_bus.slot_params;
        self.bus_slot_state = next_bus.slot_state;
        self.bus_active_slot_indices = next_bus.active_slot_indices;
        self.bus_active_slot_counts = next_bus.active_slot_counts;
        self.bus_activity_frames
            .resize(self.bus_slot_params.len(), 0);
        self.active_bus_activity_count = self
            .bus_activity_frames
            .iter()
            .filter(|frames| **frames > 0)
            .count();
        self.master_slot_params = next_master_slot_params;
        self.master_slot_state = next_master_slot_state;
        self.refresh_master_active_slot_indices();
        self.master_activity_frames = 0;
        self.bus_mono_scratch.resize(self.bus_pan_pos.len(), 0.0);
    }

    fn apply_instrument_slots_config(&mut self, instruments: Vec<InstrumentSlotConfig>) {
        for (idx, slot) in instruments.into_iter().enumerate() {
            if idx >= INSTRUMENT_SLOT_COUNT {
                break;
            }
            self.slot_kind[idx] = parse_instrument_kind(&slot.kind);
            self.apply_instrument_slot_config(idx, slot);
        }
    }

    fn apply_instrument_slot_config(&mut self, idx: usize, slot: InstrumentSlotConfig) {
        let InstrumentSlotConfig {
            kind: _,
            synth,
            mixer,
        } = slot;
        if self.slot_kind[idx] == InstrumentKind::Synth {
            self.instruments[idx] = synth;
            self.synth_render_configs[idx] = SynthVoiceRenderConfig::from_config(synth);
            self.synth_render_revisions[idx] = self.synth_render_revisions[idx].wrapping_add(1);
        }
        if let Some(mixer) = &mixer {
            self.apply_instrument_mixer_config(idx, mixer);
        }
    }

    fn apply_instrument_mixer_config(&mut self, idx: usize, mixer: &InstrumentMixerConfig) {
        self.slot_route[idx] = parse_route(&mixer.route);
        self.slot_pan_pos[idx] = mixer.pan_pos.min(self.pan_positions - 1);
        self.slot_volume[idx] = (mixer.volume / 100.0).clamp(0.0, 1.0);
    }

    fn refresh_slot_pan_gains(&mut self) {
        for idx in 0..INSTRUMENT_SLOT_COUNT {
            self.slot_pan_gains[idx] = pan_gains(self.slot_pan_pos[idx], self.pan_positions);
        }
    }

    fn compile_bus_mixer_state(&self, mixer: Option<&MixerConfig>) -> CompiledBusMixerState {
        let mut next_bus_pan_pos = Vec::new();
        let mut next_bus_pan_gains = Vec::new();
        let mut next_bus_slot_params = Vec::new();
        let mut next_bus_slot_state = Vec::new();
        let Some(mixer) = mixer else {
            return CompiledBusMixerState {
                pan_positions: next_bus_pan_pos,
                pan_gains: next_bus_pan_gains,
                slot_params: next_bus_slot_params,
                slot_state: next_bus_slot_state,
                active_slot_indices: Vec::new(),
                active_slot_counts: Vec::new(),
            };
        };
        next_bus_pan_pos.reserve_exact(mixer.buses.len());
        next_bus_pan_gains.reserve_exact(mixer.buses.len());
        next_bus_slot_params.reserve_exact(mixer.buses.len());
        next_bus_slot_state.reserve_exact(mixer.buses.len());
        let mut next_bus_active_slot_indices = Vec::with_capacity(mixer.buses.len());
        let mut next_bus_active_slot_counts = Vec::with_capacity(mixer.buses.len());
        for (bus_idx, bus) in mixer.buses.iter().enumerate() {
            let pan_pos = bus.pan_pos.min(self.pan_positions - 1);
            next_bus_pan_pos.push(pan_pos);
            next_bus_pan_gains.push(pan_gains(pan_pos, self.pan_positions));
            let cfgs = compile_bus_slot_configs(bus);
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
            let (active_indices, active_count) = active_fx_bus_slots(&params);
            next_bus_slot_params.push(params);
            next_bus_slot_state.push(states);
            next_bus_active_slot_indices.push(active_indices);
            next_bus_active_slot_counts.push(active_count);
        }
        CompiledBusMixerState {
            pan_positions: next_bus_pan_pos,
            pan_gains: next_bus_pan_gains,
            slot_params: next_bus_slot_params,
            slot_state: next_bus_slot_state,
            active_slot_indices: next_bus_active_slot_indices,
            active_slot_counts: next_bus_active_slot_counts,
        }
    }

    pub(super) fn refresh_master_active_slot_indices(&mut self) {
        self.master_active_slot_indices.clear();
        self.master_active_slot_indices
            .reserve(self.master_slot_params.len());
        for (idx, params) in self.master_slot_params.iter().enumerate() {
            if !matches!(params, FxBusParams::None) {
                self.master_active_slot_indices.push(idx);
            }
        }
    }

    fn compile_master_mixer_state(
        &self,
        mixer: Option<&MixerConfig>,
    ) -> (Vec<FxBusParams>, Vec<MasterFxState>) {
        let mut next_master_slot_params = Vec::new();
        let mut next_master_slot_state = Vec::new();
        let Some(master) = mixer.and_then(|mixer| mixer.master.as_ref()) else {
            return (next_master_slot_params, next_master_slot_state);
        };
        next_master_slot_params.reserve_exact(master.slots.len());
        next_master_slot_state.reserve_exact(master.slots.len());
        for (slot_idx, slot) in master.slots.iter().enumerate() {
            let params = compile_fx_bus_params(slot);
            let state = self
                .master_slot_state
                .get(slot_idx)
                .filter(|state| master_fx_state_matches_params(state, &params))
                .cloned()
                .unwrap_or_else(|| master_fx_state_from_params(&params));
            next_master_slot_params.push(params);
            next_master_slot_state.push(state);
        }
        (next_master_slot_params, next_master_slot_state)
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
        let (i, stole_voice) = {
            let pool = &mut self.voices[slot];
            let active = pool.iter().filter(|voice| voice.active).count();
            if active >= MAX_SYNTH_VOICES_PER_SLOT {
                (Self::steal_active_voice_index(pool), true)
            } else {
                match pool.iter().position(|voice| !voice.active) {
                    Some(i) => (i, false),
                    None => (Self::steal_active_voice_index(pool), true),
                }
            }
        };
        if stole_voice {
            self.record_voice_steal();
        }
        let cfg = self.instruments[slot];
        let amp_env = EnvState::note_on(cfg.amp_env, self.sample_rate);
        let filt_env = EnvState::note_on(cfg.filter_env, self.sample_rate);
        let mut voice = Voice {
            active: true,
            instrument_slot: slot as u8,
            midi_note,
            velocity: v,
            velocity_norm: 0.0,
            note_off_sample,
            started_sample: self.sample_clock,
            freq_hz: freq,
            osc1_inc: 0.0,
            osc2_inc: 0.0,
            render_revision: 0,
            phase1: 0.0,
            phase2: 0.0,
            amp_env,
            filt_env,
            filt: BiquadState::new(),
        };
        refresh_synth_voice_render_cache(
            &mut voice,
            &self.synth_render_configs[slot],
            self.sample_rate,
            self.synth_render_revisions[slot],
        );
        let pool = &mut self.voices[slot];
        pool[i] = voice;
        self.active_synth_slots[slot] = true;

        self.enforce_voice_budgets();
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
            self.active_sample_slots[slot] =
                self.sample_voices[slot].iter().any(|voice| voice.active);
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
        self.active_sample_slots = [false; INSTRUMENT_SLOT_COUNT];
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

fn compile_bus_slot_configs(bus: &FxBusConfig) -> [FxBusSlotConfig; BUS_SLOTS_PER_BUS] {
    let mut cfgs: [FxBusSlotConfig; BUS_SLOTS_PER_BUS] =
        std::array::from_fn(|_| FxBusSlotConfig::Kind("none".to_string()));
    for (j, slot) in bus.slots.iter().enumerate().take(BUS_SLOTS_PER_BUS) {
        cfgs[j] = slot.clone();
    }
    cfgs
}

pub(super) fn active_fx_bus_slots(
    params: &[FxBusParams; BUS_SLOTS_PER_BUS],
) -> ([usize; BUS_SLOTS_PER_BUS], usize) {
    let mut indices = [0; BUS_SLOTS_PER_BUS];
    let mut count = 0;
    for (idx, params) in params.iter().enumerate() {
        if !matches!(params, FxBusParams::None) {
            indices[count] = idx;
            count += 1;
        }
    }
    (indices, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restarting_momentary_fx_moves_it_to_end() {
        let mut engine = SynthEngine::new(44_100);
        engine.momentary_fx_start(
            "a".into(),
            "stutter".into(),
            BTreeMap::new(),
            MomentaryFxTarget::Global,
        );
        engine.momentary_fx_start(
            "b".into(),
            "freeze".into(),
            BTreeMap::new(),
            MomentaryFxTarget::Global,
        );
        engine.momentary_fx_start(
            "a".into(),
            "filter_sweep".into(),
            BTreeMap::new(),
            MomentaryFxTarget::Global,
        );

        let ids = engine
            .momentary_fx
            .iter()
            .map(|fx| fx.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["b", "a"]);
        assert!(matches!(
            engine.momentary_fx[1].kind,
            MomentaryFxKind::FilterSweep
        ));
    }
}
