use super::*;

fn frames(start: usize, count: usize) -> Vec<f32> {
    (start..start + count)
        .flat_map(|frame| [frame as f32, -(frame as f32)])
        .collect()
}

fn consume(consumer: &mut PcmMirrorConsumer, samples: usize) -> Vec<f32> {
    assert!(consumer.begin_callback());
    (0..samples)
        .map(|_| consumer.next_sample().unwrap_or(0.0))
        .collect()
}

#[test]
fn keeping_up_preserves_stereo_sample_bits() {
    let PcmMirrorPair {
        producer,
        mut consumer,
    } = new_pcm_mirror();
    let input = [1.0, -0.0, f32::from_bits(0x7fc0_1234), -2.0];
    producer.publish(&input, 2);

    let output = consume(&mut consumer, input.len());
    assert_eq!(
        output
            .iter()
            .map(|sample| sample.to_bits())
            .collect::<Vec<_>>(),
        input
            .iter()
            .map(|sample| sample.to_bits())
            .collect::<Vec<_>>()
    );
}

#[test]
fn arbitrary_callback_sizes_keep_channel_alignment_across_blocks() {
    let PcmMirrorPair {
        producer,
        mut consumer,
    } = new_pcm_mirror();
    let input = frames(0, 12);
    producer.publish(&input[..10], 5);
    let mut output = consume(&mut consumer, 6);
    producer.publish(&input[10..], 7);
    output.extend(consume(&mut consumer, 18));
    assert_eq!(output, input);
}

#[test]
fn empty_callbacks_are_silent_until_target_occupancy_is_available() {
    let PcmMirrorPair {
        producer,
        mut consumer,
    } = new_pcm_mirror();
    producer.publish(&frames(0, 1), 1);
    assert!(consumer.begin_callback());
    assert_eq!(consumer.next_sample(), Some(0.0));
    assert_eq!(consumer.next_sample(), Some(-0.0));
    assert!(!consumer.begin_callback());
    assert_eq!(consumer.next_sample(), None);

    producer.publish(
        &frames(1, PCM_MIRROR_TARGET_OCCUPANCY_FRAMES),
        PCM_MIRROR_TARGET_OCCUPANCY_FRAMES,
    );
    assert!(consumer.begin_callback());
    assert_eq!(consumer.next_sample(), Some(1.0));
}

#[test]
fn overflow_drops_oldest_and_resynchronizes_to_a_bounded_tail() {
    let PcmMirrorPair {
        producer,
        mut consumer,
    } = new_pcm_mirror();
    let input = frames(0, PCM_MIRROR_CAPACITY_FRAMES + 64);
    producer.publish(&input, PCM_MIRROR_CAPACITY_FRAMES + 64);
    assert!(!consumer.begin_callback());
    assert_eq!(consumer.next_sample(), None);
    let recovered = frames(
        PCM_MIRROR_CAPACITY_FRAMES + 64,
        PCM_MIRROR_TARGET_OCCUPANCY_FRAMES,
    );
    producer.publish(&recovered, PCM_MIRROR_TARGET_OCCUPANCY_FRAMES);
    assert!(consumer.begin_callback());
    let output = consume(&mut consumer, PCM_MIRROR_TARGET_OCCUPANCY_FRAMES * 2);
    assert_eq!(output, recovered);
}

#[test]
fn producer_rate_mismatch_never_exposes_more_than_capacity() {
    let PcmMirrorPair {
        producer,
        mut consumer,
    } = new_pcm_mirror();
    for block in 0..32 {
        let input = frames(block * 64, 64);
        producer.publish(&input, 64);
        let _ = consumer.begin_callback();
        for _ in 0..16 {
            let _ = consumer.next_sample();
        }
    }
    let write = producer.ring.write_sequence.load(Ordering::Acquire);
    assert!(frame_distance(write, consumer.read_sequence) <= PCM_MIRROR_CAPACITY_FRAMES as u64);
}

#[test]
fn invalidation_silences_existing_and_new_consumers() {
    let PcmMirrorPair {
        producer,
        mut consumer,
    } = new_pcm_mirror();
    producer.publish(&[1.0, -1.0], 1);
    producer.invalidate();
    assert!(!consumer.begin_callback());
    assert_eq!(consumer.next_sample(), None);
    assert!(!producer.new_consumer().begin_callback());

    producer.reactivate();
    assert!(!consumer.begin_callback());
    producer.publish(
        &frames(1, PCM_MIRROR_TARGET_OCCUPANCY_FRAMES),
        PCM_MIRROR_TARGET_OCCUPANCY_FRAMES,
    );
    assert!(consumer.begin_callback());
    assert_eq!(consumer.next_sample(), Some(1.0));
}

#[test]
fn generation_change_is_detected_after_the_inactive_interval_is_missed() {
    let PcmMirrorPair {
        producer,
        mut consumer,
    } = new_pcm_mirror();
    producer.publish(&frames(0, PCM_MIRROR_TARGET_OCCUPANCY_FRAMES), 256);
    assert!(consumer.begin_callback());
    assert_eq!(consumer.next_sample(), Some(0.0));
    assert_eq!(consumer.next_sample(), Some(-0.0));

    producer.invalidate();
    producer.reactivate();
    producer.publish(
        &frames(
            PCM_MIRROR_TARGET_OCCUPANCY_FRAMES,
            PCM_MIRROR_TARGET_OCCUPANCY_FRAMES,
        ),
        PCM_MIRROR_TARGET_OCCUPANCY_FRAMES,
    );

    assert!(!consumer.begin_callback());
    producer.publish(
        &frames(
            PCM_MIRROR_TARGET_OCCUPANCY_FRAMES * 2,
            PCM_MIRROR_TARGET_OCCUPANCY_FRAMES,
        ),
        PCM_MIRROR_TARGET_OCCUPANCY_FRAMES,
    );
    assert!(consumer.begin_callback());
    assert_eq!(consumer.next_sample(), Some(512.0));
}

#[test]
fn generation_wrap_is_detected_by_equality() {
    let PcmMirrorPair {
        producer,
        mut consumer,
    } = new_pcm_mirror();
    producer.ring.generation.store(u64::MAX, Ordering::Relaxed);
    consumer.observed_generation = u64::MAX;
    producer.invalidate();
    producer.reactivate();
    producer.publish(&frames(0, PCM_MIRROR_TARGET_OCCUPANCY_FRAMES), 256);

    assert!(!consumer.begin_callback());
    producer.publish(&frames(256, PCM_MIRROR_TARGET_OCCUPANCY_FRAMES), 256);
    assert!(consumer.begin_callback());
    assert_eq!(consumer.next_sample(), Some(256.0));
}

#[test]
fn concurrent_publication_never_exposes_a_mixed_stereo_frame() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};

    let PcmMirrorPair {
        producer,
        mut consumer,
    } = new_pcm_mirror();
    producer.publish(
        &frames(1, PCM_MIRROR_TARGET_OCCUPANCY_FRAMES),
        PCM_MIRROR_TARGET_OCCUPANCY_FRAMES,
    );
    let start = Arc::new(Barrier::new(2));
    let producer_started = Arc::new(AtomicBool::new(false));
    let consumer_started = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));
    let paired_frames = Arc::new(AtomicUsize::new(0));

    let writer_start = start.clone();
    let writer_started = producer_started.clone();
    let writer_consumer_started = consumer_started.clone();
    let writer_done = done.clone();
    let writer = std::thread::spawn(move || {
        writer_start.wait();
        writer_started.store(true, Ordering::Release);
        while !writer_consumer_started.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
        for block in 0..4_096 {
            let first = block * 4 + 10_000;
            let input = frames(first, 4);
            producer.publish(&input, 4);
            std::thread::yield_now();
        }
        writer_done.store(true, Ordering::Release);
    });

    let consumer_start = start;
    consumer_start.wait();
    consumer_started.store(true, Ordering::Release);
    while !done.load(Ordering::Acquire) {
        if consumer.begin_callback() {
            if let Some(left) = consumer.next_sample() {
                if let Some(right) = consumer.next_sample() {
                    assert_eq!(right.to_bits(), (-left).to_bits());
                    if producer_started.load(Ordering::Acquire) {
                        paired_frames.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
        std::thread::yield_now();
    }
    writer.join().unwrap();
    assert!(paired_frames.load(Ordering::Relaxed) > 0);
}
