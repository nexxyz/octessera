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

#[test]
fn sparse_completion_lists_and_scratch_are_exact_across_two_blocks() {
    let mut engine = prefix_engine();
    engine.note_on(0, 60, 96, 1);
    let mut synth_survivor = *engine.synth_voice_pool.lane(0).expect("synth lane");
    synth_survivor.canonical_lane = Some(2);
    synth_survivor.note_off_sample = u64::MAX;
    assert!(engine.synth_voice_pool.assign_lane(2, 0));
    *engine.synth_voice_pool.lane_mut(2).expect("synth lane") = synth_survivor;

    engine.note_on(1, 36, 96, 10_000);
    let mut sample_survivor = engine
        .sample_voice_pool
        .lane(0)
        .expect("sample lane")
        .clone();
    sample_survivor.canonical_lane = Some(2);
    sample_survivor.pos = 0.0;
    sample_survivor.step = 0.0;
    assert!(engine.sample_voice_pool.assign_lane(2, 1));
    *engine.sample_voice_pool.lane_mut(2).expect("sample lane") = sample_survivor;

    let mut synth_partition = engine
        .synth_voice_pool
        .take_partition(0)
        .expect("synth partition");
    let mut sample_partition = engine
        .sample_voice_pool
        .take_partition(0)
        .expect("sample partition");
    assert_eq!(&synth_partition.render_lanes[..2], &[0, 1]);
    assert_eq!(&sample_partition.render_lanes[..2], &[0, 1]);
    let mut synth_scratch = super::source_lane_renderer::SourceLaneBlockScratch::new();
    let mut sample_scratch = super::source_lane_renderer::SourceLaneBlockScratch::new();
    let synth_context = engine.synth_source_context();

    assert!(synth_scratch.prepare(128));
    assert!(sample_scratch.prepare(128));
    super::source_lane_renderer::reset_rendered_prefix_writes_for_test();
    super::source_lane_renderer::render_synth_partition(
        &mut synth_partition,
        128,
        0,
        &synth_context,
        &mut synth_scratch,
    );
    super::source_lane_renderer::render_sample_partition(
        &mut sample_partition,
        128,
        super::source_lane_renderer::SampleSourceContext {
            sample_rate: 48_000,
        },
        &mut sample_scratch,
    );
    assert_eq!(&synth_scratch.rendered_frames[..2], &[48, 128]);
    assert_eq!(&sample_scratch.rendered_frames[..2], &[3, 128]);
    assert_eq!(
        super::source_lane_renderer::rendered_prefix_writes_for_test(),
        [2, 2]
    );
    assert_eq!(
        &synth_partition.render_lanes[..synth_partition.render_lane_count],
        &[1]
    );
    assert_eq!(
        &sample_partition.render_lanes[..sample_partition.render_lane_count],
        &[1]
    );

    assert!(synth_scratch.prepare(64));
    assert!(sample_scratch.prepare(64));
    super::source_lane_renderer::reset_rendered_prefix_writes_for_test();
    super::source_lane_renderer::render_synth_partition(
        &mut synth_partition,
        64,
        128,
        &synth_context,
        &mut synth_scratch,
    );
    super::source_lane_renderer::render_sample_partition(
        &mut sample_partition,
        64,
        super::source_lane_renderer::SampleSourceContext {
            sample_rate: 48_000,
        },
        &mut sample_scratch,
    );
    assert_eq!(
        super::source_lane_renderer::rendered_prefix_writes_for_test(),
        [1, 1]
    );
    assert_eq!(synth_scratch.rendered_frames[0], 0);
    assert_eq!(sample_scratch.rendered_frames[0], 0);
    assert_eq!(
        synth_scratch.slots[0],
        super::source_lane_renderer::INVALID_INSTRUMENT_SLOT
    );
    assert_eq!(
        sample_scratch.slots[0],
        super::source_lane_renderer::INVALID_INSTRUMENT_SLOT
    );
    assert_eq!(&synth_scratch.rendered_frames[..2], &[0, 64]);
    assert_eq!(&sample_scratch.rendered_frames[..2], &[0, 64]);
    assert_eq!(
        &synth_partition.render_lanes[..synth_partition.render_lane_count],
        &[1]
    );
    assert_eq!(
        &sample_partition.render_lanes[..sample_partition.render_lane_count],
        &[1]
    );
}

#[test]
fn completed_prefix_reduction_does_not_leak_into_the_next_block() {
    let mut engine = prefix_engine();
    engine.note_on(0, 60, 96, 1);
    engine.note_on(1, 36, 96, 10_000);
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut output = Vec::new();

    super::source_lane_renderer::reset_rendered_prefix_writes_for_test();
    engine.render_interleaved_block(128, &mut left, &mut right, &mut output);
    assert_prefix(&engine.block_slot_scratch.synth_active[0], 48, 128);
    assert_prefix(&engine.block_slot_scratch.sample_active[1], 3, 128);
    assert_eq!(
        super::source_lane_renderer::rendered_prefix_writes_for_test(),
        [1, 1]
    );

    let synth_partition = engine
        .synth_voice_pool
        .take_partition(0)
        .expect("synth partition");
    let sample_partition = engine
        .sample_voice_pool
        .take_partition(0)
        .expect("sample partition");
    assert_eq!(synth_partition.render_lane_count, 0);
    assert_eq!(sample_partition.render_lane_count, 0);
    assert!(engine
        .synth_voice_pool
        .install_partition(0, synth_partition)
        .is_ok());
    assert!(engine
        .sample_voice_pool
        .install_partition(0, sample_partition)
        .is_ok());

    let mut second_left = Vec::new();
    let mut second_right = Vec::new();
    let mut second_output = Vec::new();
    super::source_lane_renderer::reset_rendered_prefix_writes_for_test();
    engine.render_interleaved_block(64, &mut second_left, &mut second_right, &mut second_output);
    assert!(second_output.iter().all(|sample| sample.to_bits() == 0));
    assert_prefix(&engine.block_slot_scratch.synth_active[0], 0, 64);
    assert_prefix(&engine.block_slot_scratch.sample_active[1], 0, 64);
    assert_eq!(
        super::source_lane_renderer::rendered_prefix_writes_for_test(),
        [0, 0]
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
