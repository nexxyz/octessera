use super::voice_block_sum::{sum_sample_partition, sum_synth_partition, SlotBlockSum};
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
    super::voice_block_sum::reset_rendered_voices_for_test();
    super::inline_source_executor::reset_prefix_activity_applies_for_test();
    let mut engine = prefix_engine();
    engine.note_on(0, 60, 96, 10_000);
    engine.note_on(1, 36, 96, 10_000);

    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut output = Vec::new();
    engine.render_interleaved_block(2048, &mut left, &mut right, &mut output);

    assert_eq!(super::voice_block_sum::rendered_voices_for_test(), [1, 1]);
    assert_eq!(
        super::inline_source_executor::prefix_activity_applies_for_test(),
        [INSTRUMENT_SLOT_COUNT, INSTRUMENT_SLOT_COUNT]
    );
}

#[test]
fn finished_voices_leave_the_render_lists_and_survivors_render_the_next_block() {
    let mut engine = prefix_engine();
    engine.note_on(0, 60, 96, 1);
    let mut synth_survivor = *engine.synth_voice_pool.lane(0).expect("synth lane");
    synth_survivor.note_off_sample = u64::MAX;
    assert!(engine.synth_voice_pool.assign_lane(2, 0));
    *engine.synth_voice_pool.lane_mut(2).expect("synth lane") = synth_survivor;

    engine.note_on(1, 36, 96, 10_000);
    let mut sample_survivor = engine
        .sample_voice_pool
        .lane(0)
        .expect("sample lane")
        .clone();
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
    let synth_context = engine.synth_source_context();
    let mut voice_block = vec![0.0; 128];

    for (frames, clock, rendered, prefix) in [(128, 0, [2, 2], 128), (64, 128, [1, 1], 64)] {
        let mut slot_out: [Vec<f32>; INSTRUMENT_SLOT_COUNT] =
            std::array::from_fn(|_| vec![0.0; frames]);
        let mut sum = SlotBlockSum::new(&mut slot_out);
        super::voice_block_sum::reset_rendered_voices_for_test();
        sum_synth_partition(
            &mut synth_partition,
            frames,
            clock,
            &synth_context,
            &mut voice_block,
            &mut sum,
        );
        sum_sample_partition(
            &mut sample_partition,
            frames,
            48_000,
            &mut voice_block,
            &mut sum,
        );
        assert_eq!(super::voice_block_sum::rendered_voices_for_test(), rendered);
        assert_eq!(sum.rendered_prefixes[0], prefix);
        assert_eq!(sum.rendered_prefixes[1], prefix);
        assert_eq!(
            &synth_partition.render_lanes[..synth_partition.render_lane_count],
            &[1]
        );
        assert_eq!(
            &sample_partition.render_lanes[..sample_partition.render_lane_count],
            &[1]
        );
    }
}

#[test]
fn completed_prefix_reduction_does_not_leak_into_the_next_block() {
    let mut engine = prefix_engine();
    engine.note_on(0, 60, 96, 1);
    engine.note_on(1, 36, 96, 10_000);
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut output = Vec::new();

    super::voice_block_sum::reset_rendered_voices_for_test();
    engine.render_interleaved_block(128, &mut left, &mut right, &mut output);
    assert_prefix(&engine.block_slot_scratch.synth_active[0], 48, 128);
    assert_prefix(&engine.block_slot_scratch.sample_active[1], 3, 128);
    assert_eq!(super::voice_block_sum::rendered_voices_for_test(), [1, 1]);

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
    super::voice_block_sum::reset_rendered_voices_for_test();
    engine.render_interleaved_block(64, &mut second_left, &mut second_right, &mut second_output);
    assert!(second_output.iter().all(|sample| sample.to_bits() == 0));
    assert_prefix(&engine.block_slot_scratch.synth_active[0], 0, 64);
    assert_prefix(&engine.block_slot_scratch.sample_active[1], 0, 64);
    assert_eq!(super::voice_block_sum::rendered_voices_for_test(), [0, 0]);
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
                fm: None,
                pluck: None,
                drum: None,
                kind: "synth".into(),
                synth,
                mixer: None,
            },
            InstrumentSlotConfig {
                fm: None,
                pluck: None,
                drum: None,
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
