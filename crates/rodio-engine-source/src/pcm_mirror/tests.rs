use super::*;

fn frames(start: usize, count: usize) -> Vec<f32> {
    (start..start + count)
        .flat_map(|frame| [frame as f32, -(frame as f32)])
        .collect()
}

fn consume(consumer: &mut PcmMirrorConsumer, samples: usize) -> Vec<f32> {
    assert!(consumer.begin_callback(0));
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
    let rest = PCM_MIRROR_TARGET_OCCUPANCY_FRAMES - 2;
    producer.publish(&frames(2, rest), rest);

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
    let target = PCM_MIRROR_TARGET_OCCUPANCY_FRAMES;
    let input = frames(0, target + 7);
    producer.publish(&input[..target * 2], target);
    let mut output = consume(&mut consumer, 6);
    producer.publish(&input[target * 2..], 7);
    output.extend(consume(&mut consumer, input.len() - 6));
    assert_eq!(output, input);
}

#[test]
fn a_drained_consumer_is_silent_until_target_occupancy_is_available_again() {
    let PcmMirrorPair {
        producer,
        mut consumer,
    } = new_pcm_mirror();
    let target = PCM_MIRROR_TARGET_OCCUPANCY_FRAMES;
    producer.publish(&frames(0, target), target);
    assert_eq!(consume(&mut consumer, target * 2), frames(0, target));
    assert!(!consumer.begin_callback(0));
    assert_eq!(consumer.next_sample(), None);

    producer.publish(&frames(target, target - 1), target - 1);
    assert!(!consumer.begin_callback(0));
    producer.publish(&frames(target * 2 - 1, 1), 1);
    assert!(consumer.begin_callback(0));
    assert_eq!(consumer.next_sample(), Some(target as f32));
}

#[test]
fn overflow_drops_oldest_and_resynchronizes_to_a_bounded_tail() {
    let PcmMirrorPair {
        producer,
        mut consumer,
    } = new_pcm_mirror();
    let input = frames(0, PCM_MIRROR_CAPACITY_FRAMES + 64);
    producer.publish(&input, PCM_MIRROR_CAPACITY_FRAMES + 64);
    assert!(!consumer.begin_callback(0));
    assert_eq!(consumer.next_sample(), None);
    let recovered = frames(
        PCM_MIRROR_CAPACITY_FRAMES + 64,
        PCM_MIRROR_TARGET_OCCUPANCY_FRAMES,
    );
    producer.publish(&recovered, PCM_MIRROR_TARGET_OCCUPANCY_FRAMES);
    assert!(consumer.begin_callback(0));
    let output = consume(&mut consumer, PCM_MIRROR_TARGET_OCCUPANCY_FRAMES * 2);
    assert_eq!(output, recovered);
}

fn primed_at_target(producer: &PcmMirrorProducer, consumer: &mut PcmMirrorConsumer) {
    assert!(!consumer.begin_callback(0));
    producer.publish(
        &frames(0, PCM_MIRROR_TARGET_OCCUPANCY_FRAMES),
        PCM_MIRROR_TARGET_OCCUPANCY_FRAMES,
    );
    assert!(consumer.begin_callback(0));
}

fn assert_frames_near(output: &[f32], lefts: &[f32]) {
    assert_eq!(output.len(), lefts.len() * 2);
    for (frame, left) in output.chunks(2).zip(lefts) {
        assert!((frame[0] - left).abs() < 1e-3, "{frame:?} vs {left}");
        assert!((frame[1] + left).abs() < 1e-3, "{frame:?} vs {left}");
    }
}

fn crossfaded_lefts(start: usize, toward: f32) -> Vec<f32> {
    let frames = usize::from(PCM_MIRROR_STEER_CROSSFADE_FRAMES);
    (0..frames)
        .map(|index| {
            start as f32 + index as f32 + toward * (index + 1) as f32 / (frames + 1) as f32
        })
        .collect()
}

#[test]
fn a_slow_consumer_crossfades_one_frame_out_above_the_band() {
    let PcmMirrorPair {
        producer,
        mut consumer,
    } = new_pcm_mirror();
    primed_at_target(&producer, &mut consumer);
    let extra = PCM_MIRROR_DROP_ABOVE_FRAMES as usize - PCM_MIRROR_TARGET_OCCUPANCY_FRAMES + 1;
    producer.publish(&frames(PCM_MIRROR_TARGET_OCCUPANCY_FRAMES, extra), extra);
    let fade = usize::from(PCM_MIRROR_STEER_CROSSFADE_FRAMES);

    let mut output = consume(&mut consumer, fade);
    output.extend(consume(&mut consumer, fade + 4));
    let mut expected = crossfaded_lefts(0, 1.0);
    expected.extend([fade as f32 + 1.0, fade as f32 + 2.0]);
    assert_frames_near(&output, &expected);
    assert_eq!(consume(&mut consumer, 2), frames(fade + 3, 1));
}

#[test]
fn a_fast_consumer_crossfades_one_frame_in_below_the_band() {
    let PcmMirrorPair {
        producer,
        mut consumer,
    } = new_pcm_mirror();
    primed_at_target(&producer, &mut consumer);
    let read = PCM_MIRROR_TARGET_OCCUPANCY_FRAMES - PCM_MIRROR_REPEAT_BELOW_FRAMES as usize + 1;
    let output = consume(&mut consumer, read * 2);
    assert_eq!(output, frames(0, read));
    let fade = usize::from(PCM_MIRROR_STEER_CROSSFADE_FRAMES);

    assert!(consumer.begin_callback(0));
    let target = PCM_MIRROR_TARGET_OCCUPANCY_FRAMES;
    producer.publish(&frames(target, target), target);
    let output: Vec<f32> = (0..(fade + 2) * 2)
        .map(|_| consumer.next_sample().unwrap())
        .collect();
    let mut expected = crossfaded_lefts(read, -1.0);
    expected.extend([(read + fade - 1) as f32, (read + fade) as f32]);
    assert_frames_near(&output, &expected);
}

#[test]
fn a_consumer_on_a_fresh_writer_is_silent_until_the_target() {
    let PcmMirrorPair {
        producer,
        mut consumer,
    } = new_pcm_mirror();
    let target = PCM_MIRROR_TARGET_OCCUPANCY_FRAMES;
    producer.publish(&frames(0, 8), 8);
    assert!(!consumer.begin_callback(0));
    assert_eq!(consumer.next_sample(), None);
    producer.publish(&frames(8, target - 8), target - 8);
    assert_eq!(consume(&mut consumer, 4), frames(0, 2));
}

#[test]
fn a_consumer_opened_before_the_producer_filled_primes_to_the_latest_target_frames() {
    let PcmMirrorPair {
        producer,
        mut consumer,
    } = new_pcm_mirror();
    let count = PCM_MIRROR_DROP_ABOVE_FRAMES as usize + 1;
    producer.publish(&frames(0, count), count);
    let start = count - PCM_MIRROR_TARGET_OCCUPANCY_FRAMES;
    assert_eq!(consume(&mut consumer, 4), frames(start, 2));
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
        let _ = consumer.begin_callback(0);
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
    assert!(!consumer.begin_callback(0));
    assert_eq!(consumer.next_sample(), None);
    assert!(!producer.new_consumer().begin_callback(0));

    producer.reactivate();
    assert!(!consumer.begin_callback(0));
    producer.publish(
        &frames(1, PCM_MIRROR_TARGET_OCCUPANCY_FRAMES),
        PCM_MIRROR_TARGET_OCCUPANCY_FRAMES,
    );
    assert!(consumer.begin_callback(0));
    assert_eq!(consumer.next_sample(), Some(1.0));
}

#[test]
fn generation_change_is_detected_after_the_inactive_interval_is_missed() {
    let PcmMirrorPair {
        producer,
        mut consumer,
    } = new_pcm_mirror();
    producer.publish(&frames(0, PCM_MIRROR_TARGET_OCCUPANCY_FRAMES), 256);
    assert!(consumer.begin_callback(0));
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

    assert!(!consumer.begin_callback(0));
    producer.publish(
        &frames(
            PCM_MIRROR_TARGET_OCCUPANCY_FRAMES * 2,
            PCM_MIRROR_TARGET_OCCUPANCY_FRAMES,
        ),
        PCM_MIRROR_TARGET_OCCUPANCY_FRAMES,
    );
    assert!(consumer.begin_callback(0));
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

    assert!(!consumer.begin_callback(0));
    producer.publish(&frames(256, PCM_MIRROR_TARGET_OCCUPANCY_FRAMES), 256);
    assert!(consumer.begin_callback(0));
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
        if consumer.begin_callback(0) {
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

#[test]
fn large_callbacks_keep_a_full_callback_of_headroom_above_the_target() {
    let PcmMirrorPair {
        producer,
        mut consumer,
    } = new_pcm_mirror();
    let callback = 127;
    let primed = PCM_MIRROR_TARGET_OCCUPANCY_FRAMES + callback;
    assert!(!consumer.begin_callback(callback));
    producer.publish(&frames(0, primed - 1), primed - 1);
    assert!(!consumer.begin_callback(callback));
    producer.publish(&frames(primed - 1, 1), 1);

    let mut output = Vec::new();
    for _ in 0..3 {
        assert!(consumer.begin_callback(callback));
        output.extend((0..callback * 2).map(|_| consumer.next_sample()));
    }
    assert!(output.iter().all(Option::is_some));
    assert!(!consumer.priming);
}

#[test]
fn a_consumer_created_while_the_producer_runs_primes_at_the_target() {
    let PcmMirrorPair {
        producer,
        consumer: _first,
    } = new_pcm_mirror();
    let target = PCM_MIRROR_TARGET_OCCUPANCY_FRAMES;
    producer.publish(&frames(0, target), target);
    let mut consumer = producer.new_consumer();
    let callback = 64;

    producer.publish(&frames(target, callback / 2), callback / 2);
    assert!(!consumer.begin_callback(callback));
    producer.publish(&frames(target + callback / 2, callback), callback);
    assert!(consumer.begin_callback(callback));
    assert!(!consumer.priming);
}

#[test]
fn a_callback_larger_than_the_ring_still_primes_below_capacity() {
    let PcmMirrorPair {
        producer,
        mut consumer,
    } = new_pcm_mirror();
    let callback = PCM_MIRROR_CAPACITY_FRAMES;
    let target = PCM_MIRROR_CAPACITY_FRAMES - usize::from(PCM_MIRROR_STEER_CROSSFADE_FRAMES);
    producer.publish(&frames(0, target - 1), target - 1);
    assert!(!consumer.begin_callback(callback));
    producer.publish(&frames(target - 1, 1), 1);
    assert!(consumer.begin_callback(callback));
    assert_eq!(consumer.next_sample(), Some(0.0));
}
