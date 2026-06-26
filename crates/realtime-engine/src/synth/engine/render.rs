use super::*;
use crate::simd::interleave_stereo;

impl SynthEngine {
    pub fn next_sample(&mut self) -> f32 {
        let (l, r) = self.next_stereo_sample();
        (l + r) * 0.5
    }

    pub fn next_stereo_sample(&mut self) -> (f32, f32) {
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
        for frame in 0..frames {
            let (l, r) = self.next_stereo_sample();
            left[frame] = l;
            right[frame] = r;
        }
        interleave_stereo(left, right, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::{
        FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig, InstrumentSlotConfig,
        InstrumentsConfig, MixerConfig,
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
}
