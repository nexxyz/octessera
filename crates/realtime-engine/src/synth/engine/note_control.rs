use super::*;

impl SynthEngine {
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

    pub fn set_sample_bank(&mut self, instrument_slot: usize, bank: SampleBankConfig) {
        let slot = instrument_slot.min(INSTRUMENT_SLOT_COUNT - 1);
        self.sample_banks
            .resize(INSTRUMENT_SLOT_COUNT, SampleBankConfig::default());
        self.sample_banks[slot] = bank;
        for voice in self.sample_voices[slot].iter_mut() {
            voice.active = false;
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
            filt: BiquadState::new(),
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
