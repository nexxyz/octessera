use super::*;

#[test]
fn take_derives_sorted_sparse_parity_local_render_lanes() {
    let mut pool = SynthVoicePool::new();
    let capacity = SYNTH_VOICE_LANE_CAPACITY;
    for (lane, slot) in [
        (0, 0),
        (1, 1),
        (3, 2),
        (6, 3),
        (capacity - 2, 4),
        (capacity - 1, 5),
    ] {
        assert!(pool.assign_lane(lane, slot));
        pool.lane_mut(lane).expect("home partition lane").active = true;
    }

    let first = pool.take_partition(0).expect("partition 0 home");
    assert_eq!(
        &first.render_lanes[..first.render_lane_count],
        &[0, 3, capacity / 2 - 1]
    );
    assert!(pool.install_partition(0, first).is_ok());

    let second = pool.take_partition(1).expect("partition 1 home");
    assert_eq!(
        &second.render_lanes[..second.render_lane_count],
        &[0, 1, capacity / 2 - 1]
    );
    assert!(pool.install_partition(1, second).is_ok());
}

#[test]
fn retake_rebuilds_render_lanes_after_ownership_changes() {
    let mut pool = SynthVoicePool::new();
    for lane in [0, 4] {
        assert!(pool.assign_lane(lane, 0));
        pool.lane_mut(lane).expect("home partition lane").active = true;
    }
    let partition = pool.take_partition(0).expect("partition 0 home");
    assert_eq!(
        &partition.render_lanes[..partition.render_lane_count],
        &[0, 2]
    );
    assert!(pool.install_partition(0, partition).is_ok());

    pool.lane_mut(4).expect("home partition lane").active = false;
    assert!(pool.compact_slot_lanes(0));
    assert!(pool.assign_lane(6, 1));
    pool.lane_mut(6).expect("home partition lane").active = true;

    let partition = pool.take_partition(0).expect("partition 0 home");
    assert_eq!(
        &partition.render_lanes[..partition.render_lane_count],
        &[0, 3]
    );
}
