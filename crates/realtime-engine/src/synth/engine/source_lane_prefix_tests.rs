use super::*;

#[test]
fn rendered_prefixes_match_synth_and_sample_completion_and_serial_output() {
    for frames in [32, 64, 128, 256, 2048] {
        let mut block = prefix_engine();
        let mut serial = prefix_engine();
        block.note_on(0, 60, 96, 1);
        serial.note_on(0, 60, 96, 1);
        block.note_on(1, 36, 96, 10_000);
        serial.note_on(1, 36, 96, 10_000);

        let mut left = Vec::new();
        let mut right = Vec::new();
        let mut output = Vec::new();
        block.render_interleaved_block(frames, &mut left, &mut right, &mut output);
        let mut expected = Vec::with_capacity(frames * 2);
        for _ in 0..frames {
            let (left, right) = serial.next_stereo_sample();
            expected.push(left);
            expected.push(right);
        }
        assert_eq!(output.len(), expected.len());
        for (index, (actual, expected)) in output.iter().zip(expected).enumerate() {
            assert_eq!(actual.to_bits(), expected.to_bits(), "sample {index}");
        }

        assert_prefix(&block.block_slot_scratch.synth_active[0], 48, frames);
        assert_prefix(&block.block_slot_scratch.sample_active[1], 3, frames);
        assert_eq!(block.active_synth_slots[0], frames < 48);
        assert_eq!(block.active_sample_slots[1], frames < 3);
    }
}

#[test]
fn prefix_metadata_and_activity_application_are_not_per_frame_operations() {
    super::source_lane_renderer::reset_rendered_prefix_writes_for_test();
    super::inline_source_executor::reset_prefix_activity_applies_for_test();
    let mut engine = prefix_engine();
    engine.note_on(0, 60, 96, 10_000);
    engine.note_on(1, 36, 96, 10_000);

    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut output = Vec::new();
    engine.render_interleaved_block(2048, &mut left, &mut right, &mut output);

    assert_eq!(
        super::source_lane_renderer::rendered_prefix_writes_for_test(),
        [1, 1]
    );
    assert_eq!(
        super::inline_source_executor::prefix_activity_applies_for_test(),
        [INSTRUMENT_SLOT_COUNT, INSTRUMENT_SLOT_COUNT]
    );
}

fn prefix_engine() -> SynthEngine {
    let mut synth = default_synth_config();
    synth.amp_env.attack_ms = 0.0;
    synth.amp_env.decay_ms = 0.0;
    synth.amp_env.release_ms = 0.0;
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![
            InstrumentSlotConfig {
                kind: "synth".into(),
                synth,
                mixer: None,
            },
            InstrumentSlotConfig {
                kind: "sampler".into(),
                synth,
                mixer: None,
            },
        ],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: vec![1.0, -0.5, 0.25].into(),
            channels: 1,
            sample_rate: 48_000,
        }),
    };
    let _ = engine.set_sample_banks(vec![SampleBankConfig::default(), bank]);
    engine
}

fn assert_prefix(active: &[bool], prefix: usize, frames: usize) {
    assert!(active[..prefix.min(frames)].iter().all(|active| *active));
    assert!(active[prefix.min(frames)..frames]
        .iter()
        .all(|active| !*active));
}
