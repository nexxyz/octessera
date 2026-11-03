use super::super::test_allocator::count_allocations_and_deallocations;
use super::*;

fn block(frames: usize, first: f32) -> Vec<f32> {
    (0..frames)
        .flat_map(|frame| {
            let value = first + frame as f32;
            [value, value + 0.5]
        })
        .collect()
}

#[test]
fn an_unselected_tap_takes_nothing() {
    let tap = SignalTap::new();
    tap.publish_master(&block(128, 1.0), 128, 48_000);
    assert_eq!(tap.written(), 0);
    assert_eq!(tap.sample_rate(), 0);
    let mut out = [9.0_f32; 4];
    assert_eq!(tap.read_latest(0, &mut out), 0);
    assert_eq!(out, [0.0; 4]);

    tap.select(Some(SignalSource::Master));
    tap.select(None);
    tap.publish_master(&block(128, 1.0), 128, 48_000);
    assert_eq!(tap.written(), 0);
}

#[test]
fn the_master_tap_keeps_the_mono_average_of_each_frame_pair() {
    let tap = SignalTap::new();
    tap.select(Some(SignalSource::Master));
    tap.publish_master(&block(8, 0.0), 8, 48_000);
    assert_eq!(tap.written(), 4);
    assert_eq!(tap.sample_rate(), 24_000);
    let mut out = [0.0_f32; 6];
    assert_eq!(tap.read_latest(0, &mut out), 4);
    assert_eq!(out, [0.0, 0.0, 0.75, 2.75, 4.75, 6.75]);
}

#[test]
fn reads_count_only_samples_after_since_and_wrap_around_the_ring() {
    let tap = SignalTap::new();
    tap.select(Some(SignalSource::Master));
    let frames = SIGNAL_TAP_CAPACITY * SIGNAL_TAP_DECIMATION;
    tap.publish_master(&block(frames, 0.0), frames, 48_000);
    let since = tap.written();
    tap.publish_master(&block(20, 10_000.0), 20, 48_000);
    assert_eq!(tap.written(), since + 10);

    let mut out = [0.0_f32; 16];
    assert_eq!(tap.read_latest(since, &mut out), 10);
    assert_eq!(out[..6], [0.0; 6]);
    assert_eq!(out[6], 10_000.75);
    assert_eq!(out[15], 10_018.75);

    let mut all = vec![0.0_f32; SIGNAL_TAP_CAPACITY + 8];
    assert_eq!(tap.read_latest(0, &mut all), SIGNAL_TAP_CAPACITY);
    assert_eq!(all[..8], [0.0; 8]);
    assert_eq!(*all.last().unwrap(), 10_018.75);
    assert_eq!(
        all[8 + SIGNAL_TAP_CAPACITY - 11],
        (frames - 2) as f32 + 0.75
    );
}

#[test]
fn publishing_a_block_allocates_nothing() {
    let tap = SignalTap::new();
    tap.select(Some(SignalSource::Master));
    let samples = block(256, 0.0);
    let ((), allocations, deallocations) = count_allocations_and_deallocations(|| {
        tap.publish_master(&samples, 256, 48_000);
        tap.select(None);
        tap.publish_master(&samples, 256, 48_000);
    });
    assert_eq!((allocations, deallocations), (0, 0));
}
