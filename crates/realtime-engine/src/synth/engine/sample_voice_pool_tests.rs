use super::*;

const TEST_LANES: usize = 9;
const _: () = assert!(SAMPLE_VOICE_LANE_CAPACITY >= TEST_LANES);

#[test]
fn one_slot_can_assign_and_iterate_more_than_eight_lanes() {
    let mut pool = SampleVoicePool::new();
    for lane in 0..TEST_LANES {
        assert!(pool.assign_lane(lane, 0));
        pool.lane_mut(lane).expect("home partition lane").active = true;
    }

    assert_eq!(
        pool.slot_lanes(0).unwrap(),
        (0..TEST_LANES).collect::<Vec<_>>()
    );
    assert_eq!(pool.active_count_for_slot(0), Some(TEST_LANES));
    pool.assert_invariants();
}

#[test]
fn repeated_assignment_compaction_clearing_and_reuse_preserve_invariants() {
    let mut pool = SampleVoicePool::new();
    for round in 0..32 {
        for lane in 0..SAMPLE_VOICE_LANE_CAPACITY {
            let slot = (lane + round) % INSTRUMENT_SLOT_COUNT;
            assert!(pool.assign_lane(lane, slot));
            let voice = pool.lane_mut(lane).expect("home partition lane");
            voice.instrument_slot = slot as u8;
            voice.active = true;
        }
        for lane in 0..SAMPLE_VOICE_LANE_CAPACITY {
            if (lane + round) % 3 == 0 {
                pool.lane_mut(lane).expect("home partition lane").active = false;
            }
        }
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            assert!(pool.compact_slot_lanes(slot));
        }
        pool.assert_invariants();
    }

    assert!(pool.clear_slot(0).is_some());
    pool.assert_invariants();
    assert!(pool.clear_all().is_some());
    pool.assert_invariants();

    for lane in 0..TEST_LANES {
        assert!(pool.assign_lane(lane, 0));
        let voice = pool.lane_mut(lane).expect("home partition lane");
        voice.instrument_slot = 0;
        voice.active = true;
    }
    pool.assert_invariants();
}

#[test]
fn parity_mapping_is_complete_and_disjoint() {
    let mut seen = [false; SAMPLE_VOICE_LANE_CAPACITY];
    for (lane, mapped) in seen.iter_mut().enumerate() {
        let (parity, local) = partition_lane(lane).expect("mapped lane");
        assert_eq!(parity, lane % 2);
        assert_eq!(local, lane / 2);
        assert!(!*mapped);
        *mapped = true;
    }
    assert!(seen.into_iter().all(|mapped| mapped));
}

#[test]
fn partition_take_install_rejects_wrong_duplicate_and_missing_ownership() {
    let mut pool = SampleVoicePool::new();
    let partition = pool.take_partition(0).expect("partition 0 home");
    assert!(!pool.has_home());
    assert!(pool.lane(0).is_none());
    assert!(pool.slot_lanes(0).is_none());
    assert!(!pool.assign_lane(0, 0));
    let partition = pool
        .install_partition(1, partition)
        .expect_err("wrong parity must be rejected");
    assert!(pool.install_partition(0, partition).is_ok());
    assert!(pool
        .install_partition(0, Box::new(SampleVoicePartition::new(0)))
        .is_err());
    assert!(pool.has_home());
    assert!(pool.take_partition(2).is_none());
}

#[test]
fn fixed_retirement_holds_every_physical_sample_lane() {
    let mut pool = SampleVoicePool::new();
    for lane in 0..SAMPLE_VOICE_LANE_CAPACITY {
        let slot = lane % INSTRUMENT_SLOT_COUNT;
        assert!(pool.assign_lane(lane, slot));
        let voice = pool.lane_mut(lane).expect("home partition lane");
        voice.active = true;
        voice.instrument_slot = slot as u8;
        voice.buffer = Some(SampleBuffer {
            samples: vec![lane as f32].into(),
            channels: 1,
            sample_rate: 48_000,
        });
    }

    let retired = pool.clear_all().expect("home partition lanes");

    assert_eq!(retired.len(), SAMPLE_VOICE_LANE_CAPACITY);
    assert_eq!(pool.active_total(), Some(0));
}
