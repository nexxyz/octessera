use super::render_profile;
use super::*;
use crate::simd::interleave_stereo;
use std::time::Instant;

impl SynthEngine {
    pub fn next_sample(&mut self) -> f32 {
        let (l, r) = self.next_stereo_sample();
        (l + r) * 0.5
    }

    pub fn next_stereo_sample(&mut self) -> (f32, f32) {
        if self.render_profile.enabled {
            return self.profiled_serial_frame_graph();
        }
        self.serial_frame_graph()
    }

    fn serial_frame_graph(&mut self) -> (f32, f32) {
        let mut slot_out = [0.0_f32; INSTRUMENT_SLOT_COUNT];
        let sample_active = self.render_sample_voices(&mut slot_out);
        let preview_active = self.render_preview_sample_voices(&mut slot_out);
        let synth_active = self.render_synth_voices(&mut slot_out);
        let process_buses = self.should_process_fx_buses();
        if process_buses {
            self.prepare_bus_buffers();
        }
        let (mut left, mut right) = self.mix_instrument_slots(&slot_out);
        if process_buses {
            (left, right) = self.mix_fx_buses(&slot_out, left, right);
        }
        self.push_dry_history(left, right);
        let master_signal = self.signal_present(left, right)
            || synth_active
            || sample_active
            || preview_active
            || !self.momentary_fx.is_empty()
            || self.active_bus_activity_count > 0;
        let master_active = master_signal || self.master_activity_frames > 0;
        if master_active {
            (left, right) = self.apply_master_fx_slots(left, right);
            (left, right) =
                self.process_momentary_fx_target(MomentaryFxTarget::Global, left, right);
            self.master_activity_frames = if master_signal || self.signal_present(left, right) {
                self.fx_activity_hold_frames
            } else {
                self.master_activity_frames.saturating_sub(1)
            };
        }
        self.sample_clock = self.sample_clock.saturating_add(1);
        (
            (left * self.master_volume).clamp(-1.0, 1.0),
            (right * self.master_volume).clamp(-1.0, 1.0),
        )
    }

    fn profiled_serial_frame_graph(&mut self) -> (f32, f32) {
        let frame_start = Instant::now();
        let mut slot_out = [0.0_f32; INSTRUMENT_SLOT_COUNT];

        let start = Instant::now();
        let sample_active = self.render_sample_voices(&mut slot_out);
        self.render_profile.stage_ns[render_profile::PROFILE_SAMPLE_VOICES] =
            start.elapsed().as_nanos() as u64;

        let start = Instant::now();
        let preview_active = self.render_preview_sample_voices(&mut slot_out);
        self.render_profile.stage_ns[render_profile::PROFILE_PREVIEW_SAMPLE_VOICES] =
            start.elapsed().as_nanos() as u64;

        let start = Instant::now();
        let synth_active = self.render_synth_voices(&mut slot_out);
        self.render_profile.stage_ns[render_profile::PROFILE_SYNTH_VOICES] =
            start.elapsed().as_nanos() as u64;

        let start = Instant::now();
        let process_buses = self.should_process_fx_buses();
        if process_buses {
            self.prepare_bus_buffers();
        }
        let (mut left, mut right) = self.mix_instrument_slots(&slot_out);
        self.render_profile.stage_ns[render_profile::PROFILE_PREPARE_MIX_SLOTS] =
            start.elapsed().as_nanos() as u64;

        let start = Instant::now();
        if process_buses {
            (left, right) = self.mix_fx_buses(&slot_out, left, right);
        }
        self.render_profile.stage_ns[render_profile::PROFILE_FX_BUSES] =
            start.elapsed().as_nanos() as u64;

        let start = Instant::now();
        self.push_dry_history(left, right);
        self.render_profile.stage_ns[render_profile::PROFILE_DRY_HISTORY] =
            start.elapsed().as_nanos() as u64;

        let start = Instant::now();
        let master_signal = self.signal_present(left, right)
            || synth_active
            || sample_active
            || preview_active
            || !self.momentary_fx.is_empty()
            || self.active_bus_activity_count > 0;
        let master_active = master_signal || self.master_activity_frames > 0;
        if master_active {
            (left, right) = self.apply_master_fx_slots(left, right);
            (left, right) =
                self.process_momentary_fx_target(MomentaryFxTarget::Global, left, right);
            self.master_activity_frames = if master_signal || self.signal_present(left, right) {
                self.fx_activity_hold_frames
            } else {
                self.master_activity_frames.saturating_sub(1)
            };
        }
        self.render_profile.stage_ns[render_profile::PROFILE_MASTER_GLOBAL_FX] =
            start.elapsed().as_nanos() as u64;

        let start = Instant::now();
        self.sample_clock = self.sample_clock.saturating_add(1);
        let out = (
            (left * self.master_volume).clamp(-1.0, 1.0),
            (right * self.master_volume).clamp(-1.0, 1.0),
        );
        self.render_profile.stage_ns[render_profile::PROFILE_CLOCK_VOLUME_CLAMP] =
            start.elapsed().as_nanos() as u64;
        self.render_profile.frames_observed = self.render_profile.frames_observed.saturating_add(1);
        self.render_profile.last_frame_total_ns = frame_start.elapsed().as_nanos() as u64;
        out
    }

    pub fn render_interleaved_block(
        &mut self,
        frames: usize,
        left: &mut Vec<f32>,
        right: &mut Vec<f32>,
        out: &mut Vec<f32>,
    ) {
        left.resize(frames, 0.0);
        right.resize(frames, 0.0);
        out.resize(frames * 2, 0.0);
        let block_start = self.render_profile.enabled.then(Instant::now);
        for frame in 0..frames {
            let (l, r) = self.next_stereo_sample();
            left[frame] = l;
            right[frame] = r;
        }
        let interleave_start = self.render_profile.enabled.then(Instant::now);
        interleave_stereo(left, right, out);
        if let Some(start) = interleave_start {
            self.render_profile.interleave_ns = start.elapsed().as_nanos() as u64;
            self.render_profile.blocks_observed =
                self.render_profile.blocks_observed.saturating_add(1);
            self.render_profile.last_block_frames = frames;
            self.render_profile.last_block_total_ns = block_start
                .map(|block_start| block_start.elapsed().as_nanos() as u64)
                .unwrap_or(self.render_profile.interleave_ns);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::{
        FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig, InstrumentSlotConfig,
        InstrumentsConfig, MixerConfig, SampleBankConfig, SampleBuffer, SampleSlotConfig,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn block_render_matches_repeated_stereo_samples_for_synth() {
        let mut block = SynthEngine::new(44_100);
        let mut reference = SynthEngine::new(44_100);
        block.note_on(0, 60, 96, 1_000);
        reference.note_on(0, 60, 96, 1_000);
        assert_block_matches_reference(block, reference, 128);
    }

    #[test]
    fn block_render_matches_repeated_stereo_samples_for_fx() {
        let config = delay_bus_config();
        let mut block = SynthEngine::new(44_100);
        let mut reference = SynthEngine::new(44_100);
        block.set_instruments(config.clone());
        reference.set_instruments(config);
        block.note_on(0, 60, 96, 1_000);
        reference.note_on(0, 60, 96, 1_000);
        assert_block_matches_reference(block, reference, 256);
    }

    #[test]
    fn block_render_matches_repeated_stereo_samples_for_multi_slot_synth() {
        let mut block = SynthEngine::new(44_100);
        let mut reference = SynthEngine::new(44_100);
        for (slot, note, velocity) in [(0, 60, 96), (1, 64, 88), (2, 67, 104), (3, 72, 72)] {
            block.note_on(slot, note, velocity, 1_000);
            reference.note_on(slot, note, velocity, 1_000);
        }

        assert_block_matches_reference(block, reference, 256);
    }

    #[test]
    fn block_render_matches_repeated_stereo_samples_for_multi_slot_samples() {
        let mut block = multi_slot_sample_engine();
        let mut reference = multi_slot_sample_engine();
        for (slot, velocity) in [(0, 127), (1, 96), (2, 80), (3, 112)] {
            block.note_on(slot, 36, velocity, 1_000);
            reference.note_on(slot, 36, velocity, 1_000);
        }
        assert_eq!(block.profile_snapshot().active_sample_voices, 4);
        assert_eq!(reference.profile_snapshot().active_sample_voices, 4);

        assert_block_matches_reference(block, reference, 8);
    }

    #[test]
    fn note_on_keeps_synth_voice_instrument_slot_aligned_with_pool() {
        let mut engine = SynthEngine::new(44_100);
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            engine.note_on(slot as u8, 60 + slot as u8, 96, 1_000);
        }

        for slot in 0..INSTRUMENT_SLOT_COUNT {
            assert!(engine.voices[slot]
                .iter()
                .filter(|voice| voice.active)
                .all(|voice| voice.instrument_slot as usize == slot));
        }
    }

    #[test]
    fn render_profile_disabled_remains_inert_after_rendering() {
        let mut engine = SynthEngine::new(44_100);
        let mut left = Vec::new();
        let mut right = Vec::new();
        let mut out = Vec::new();
        engine.note_on(0, 60, 96, 1_000);

        let _ = engine.next_stereo_sample();
        engine.render_interleaved_block(16, &mut left, &mut right, &mut out);

        assert_eq!(
            engine.render_profile_snapshot(),
            RenderProfileSnapshot::default()
        );
    }

    #[test]
    fn render_profile_enabled_records_block_observations() {
        let mut engine = SynthEngine::new(44_100);
        let mut left = Vec::new();
        let mut right = Vec::new();
        let mut out = Vec::new();
        engine.set_render_profile_enabled(true);
        engine.note_on(0, 60, 96, 1_000);

        engine.render_interleaved_block(32, &mut left, &mut right, &mut out);

        let snapshot = engine.render_profile_snapshot();
        assert!(snapshot.enabled);
        assert_eq!(snapshot.frames_observed, 32);
        assert_eq!(snapshot.blocks_observed, 1);
        assert_eq!(snapshot.last_block_frames, 32);
        assert!(snapshot.last_frame_total_ns > 0);
        assert!(snapshot.last_block_total_ns > 0);
        assert_eq!(snapshot.stage_ns.len(), RENDER_PROFILE_STAGE_COUNT);
    }

    #[test]
    fn profiled_block_render_matches_unprofiled_fx_reference() {
        let config = delay_bus_config();
        let mut profiled = SynthEngine::new(44_100);
        let mut reference = SynthEngine::new(44_100);
        profiled.set_instruments(config.clone());
        reference.set_instruments(config);
        profiled.set_render_profile_enabled(true);
        profiled.note_on(0, 60, 96, 1_000);
        reference.note_on(0, 60, 96, 1_000);

        assert_block_matches_reference(profiled, reference, 256);
    }

    fn assert_block_matches_reference(
        mut block: SynthEngine,
        mut reference: SynthEngine,
        frames: usize,
    ) {
        let mut left = Vec::new();
        let mut right = Vec::new();
        let mut out = Vec::new();
        block.render_interleaved_block(frames, &mut left, &mut right, &mut out);
        let mut expected = Vec::with_capacity(frames * 2);
        for _ in 0..frames {
            let (l, r) = reference.next_stereo_sample();
            expected.push(l);
            expected.push(r);
        }
        assert_eq!(out.len(), expected.len());
        for (idx, (actual, expected)) in out.iter().zip(expected).enumerate() {
            assert_eq!(actual.to_bits(), expected.to_bits(), "sample {idx}");
        }
    }

    fn delay_bus_config() -> InstrumentsConfig {
        let synth = default_synth_config();
        InstrumentsConfig {
            instruments: vec![InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth,
                mixer: Some(InstrumentMixerConfig {
                    route: "fx_bus_1".to_string(),
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume: 100.0,
                }),
            }],
            mixer: Some(MixerConfig {
                buses: vec![FxBusConfig {
                    slots: vec![FxBusSlotConfig::Config {
                        kind: "delay".to_string(),
                        params: [
                            ("timeMs".to_string(), json!(35.0)),
                            ("feedback".to_string(), json!(0.25)),
                            ("mixPct".to_string(), json!(35.0)),
                        ]
                        .into_iter()
                        .collect::<BTreeMap<_, _>>(),
                    }],
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                }],
                master: None,
            }),
            pan_positions: DEFAULT_PAN_POSITIONS,
            master_volume: 100.0,
        }
    }

    fn multi_slot_sample_engine() -> SynthEngine {
        let mut engine = SynthEngine::new(48_000);
        engine.set_instruments(InstrumentsConfig {
            instruments: (0..INSTRUMENT_SLOT_COUNT)
                .map(|_| InstrumentSlotConfig {
                    kind: "sampler".to_string(),
                    synth: default_synth_config(),
                    mixer: None,
                })
                .collect(),
            mixer: None,
            pan_positions: DEFAULT_PAN_POSITIONS,
            master_volume: 100.0,
        });
        engine.set_sample_banks(
            (0..INSTRUMENT_SLOT_COUNT)
                .map(|slot| sample_bank(vec![1.0 - slot as f32 * 0.1, 0.5, 0.25, 0.0]))
                .collect(),
        );
        engine
    }

    fn sample_bank(samples: Vec<f32>) -> SampleBankConfig {
        let mut bank = SampleBankConfig::default();
        bank.slots[0] = SampleSlotConfig {
            buffer: Some(SampleBuffer {
                samples: samples.into(),
                channels: 1,
                sample_rate: 48_000,
            }),
        };
        bank
    }
}
