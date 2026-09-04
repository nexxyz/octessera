use super::*;
use crate::synth::engine::SynthEngine;

#[test]
fn active_invalid_slot_voices_survive_sparse_source_render() {
    let mut engine = SynthEngine::new(48_000);
    assert!(engine.synth_voice_pool.assign_lane(4, 0));
    engine
        .synth_voice_pool
        .lane_mut(4)
        .expect("synth lane")
        .instrument_slot = INVALID_INSTRUMENT_SLOT;
    engine
        .synth_voice_pool
        .lane_mut(4)
        .expect("synth lane")
        .active = true;
    assert!(engine.sample_voice_pool.assign_lane(4, 0));
    engine
        .sample_voice_pool
        .lane_mut(4)
        .expect("sample lane")
        .instrument_slot = INVALID_INSTRUMENT_SLOT;
    engine
        .sample_voice_pool
        .lane_mut(4)
        .expect("sample lane")
        .active = true;

    let mut synth_partition = engine
        .synth_voice_pool
        .take_partition(0)
        .expect("synth home");
    let mut sample_partition = engine
        .sample_voice_pool
        .take_partition(0)
        .expect("sample home");
    let mut synth_scratch = SourceLaneBlockScratch::new();
    let mut sample_scratch = SourceLaneBlockScratch::new();
    assert!(synth_scratch.prepare(32));
    assert!(sample_scratch.prepare(32));
    let synth_context = engine.synth_source_context();

    render_synth_partition(
        &mut synth_partition,
        32,
        0,
        &synth_context,
        &mut synth_scratch,
    );
    render_sample_partition(
        &mut sample_partition,
        32,
        SampleSourceContext {
            sample_rate: 48_000,
        },
        &mut sample_scratch,
    );

    assert_eq!(synth_partition.render_lane_count, 1);
    assert_eq!(synth_partition.render_lanes[0], 2);
    assert_eq!(sample_partition.render_lane_count, 1);
    assert_eq!(sample_partition.render_lanes[0], 2);
    assert_eq!(synth_scratch.slots[2], INVALID_INSTRUMENT_SLOT);
    assert_eq!(sample_scratch.slots[2], INVALID_INSTRUMENT_SLOT);
}
