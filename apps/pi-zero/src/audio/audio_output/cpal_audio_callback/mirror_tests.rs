use super::{fill_mirror_callback_with_scheduler, MirrorCallbackSource};
use crate::audio_priority::{
    install_test_scheduling, CallbackSchedulingHandle, CpuMask, InjectedSchedulingOutcomes,
    SchedulingFailureStage, SchedulingSyscall,
};
use rodio_engine_source::new_pcm_mirror;

#[test]
fn mirror_callback_preserves_stereo_for_arbitrary_callback_sizes() {
    let _guard = install_test_scheduling(InjectedSchedulingOutcomes::success_for_cpu(0));
    let pair = new_pcm_mirror();
    let producer = pair.producer;
    let mut callback_source = MirrorCallbackSource::new(pair.consumer);
    producer.publish(&[1.0, -1.0, 2.0, -2.0, 3.0, -3.0], 3);

    let mut first = [0.0_f32; 2];
    fill_mirror_callback_with_scheduler(
        &mut first,
        &mut callback_source,
        &CallbackSchedulingHandle::new_mirror(),
    );
    let mut second = [0.0_f32; 4];
    fill_mirror_callback_with_scheduler(
        &mut second,
        &mut callback_source,
        &CallbackSchedulingHandle::new_mirror(),
    );

    assert_eq!(first, [1.0, -1.0]);
    assert_eq!(second, [2.0, -2.0, 3.0, -3.0]);
}

#[test]
fn mirror_consumer_preserves_right_phase_when_producer_publishes_between_channels() {
    let pair = new_pcm_mirror();
    let producer = pair.producer;
    let mut consumer = pair.consumer;
    producer.publish(&[1.0, -1.0].repeat(256), 256);

    assert!(consumer.begin_callback());
    assert_eq!(consumer.next_sample(), Some(1.0));
    producer.publish(&[99.0, -99.0], 1);
    assert_eq!(consumer.next_sample(), Some(-1.0));
}

#[test]
fn mirror_consumer_silences_the_remainder_after_mid_callback_overflow() {
    let pair = new_pcm_mirror();
    let producer = pair.producer;
    let mut consumer = pair.consumer;
    producer.publish(&[1.0, -1.0].repeat(256), 256);

    assert!(consumer.begin_callback());
    assert_eq!(consumer.next_sample(), Some(1.0));
    producer.publish(
        &[2.0, -2.0].repeat(rodio_engine_source::PCM_MIRROR_CAPACITY_FRAMES + 1),
        rodio_engine_source::PCM_MIRROR_CAPACITY_FRAMES + 1,
    );
    assert_eq!(consumer.next_sample(), None);
    assert_eq!(consumer.next_sample(), None);

    producer.publish(
        &[3.0, -3.0].repeat(rodio_engine_source::PCM_MIRROR_TARGET_OCCUPANCY_FRAMES),
        rodio_engine_source::PCM_MIRROR_TARGET_OCCUPANCY_FRAMES,
    );
    assert!(consumer.begin_callback());
    assert_eq!(consumer.next_sample(), Some(3.0));
}

#[test]
fn mirror_callback_rejects_odd_output_without_consuming_a_left_sample_into_right_phase() {
    let _guard = install_test_scheduling(InjectedSchedulingOutcomes::success_for_cpu(0));
    let pair = new_pcm_mirror();
    let producer = pair.producer;
    let mut callback_source = MirrorCallbackSource::new(pair.consumer);
    let scheduler = CallbackSchedulingHandle::new_mirror();
    producer.publish(&[4.0, -4.0].repeat(256), 256);

    let mut odd_output = [1.0_f32; 3];
    fill_mirror_callback_with_scheduler(&mut odd_output, &mut callback_source, &scheduler);
    assert!(odd_output.iter().all(|sample| sample.to_bits() == 0));

    let mut output = [0.0_f32; 2];
    fill_mirror_callback_with_scheduler(&mut output, &mut callback_source, &scheduler);
    assert_eq!(output, [4.0, -4.0]);
}

#[test]
fn mirror_callback_recovers_on_a_complete_channel_pair_after_underrun() {
    let _guard = install_test_scheduling(InjectedSchedulingOutcomes::success_for_cpu(0));
    let pair = new_pcm_mirror();
    let producer = pair.producer;
    let mut callback_source = MirrorCallbackSource::new(pair.consumer);
    let scheduler = CallbackSchedulingHandle::new_mirror();
    producer.publish(&[5.0, -5.0], 1);

    let mut output = [1.0_f32; 4];
    fill_mirror_callback_with_scheduler(&mut output, &mut callback_source, &scheduler);
    assert_eq!(output, [5.0, -5.0, 0.0, 0.0]);

    producer.publish(
        &[6.0, -6.0].repeat(rodio_engine_source::PCM_MIRROR_TARGET_OCCUPANCY_FRAMES),
        rodio_engine_source::PCM_MIRROR_TARGET_OCCUPANCY_FRAMES,
    );
    let mut recovered = [0.0_f32; 2];
    fill_mirror_callback_with_scheduler(&mut recovered, &mut callback_source, &scheduler);
    assert_eq!(recovered, [6.0, -6.0]);
}

#[test]
fn mirror_callback_scheduling_failure_is_route_local_and_silent() {
    let pair = new_pcm_mirror();
    let producer = pair.producer;
    let mut callback_source = MirrorCallbackSource::new(pair.consumer);
    let mut outcomes = InjectedSchedulingOutcomes::success_for_cpu(0);
    outcomes.observed_affinity = Some(CpuMask::single(0) | CpuMask::single(1));
    let guard = install_test_scheduling(outcomes);
    let scheduler = CallbackSchedulingHandle::new_mirror();
    producer.publish(&[7.0, -7.0].repeat(256), 256);

    let mut output = [1.0_f32; 2];
    fill_mirror_callback_with_scheduler(&mut output, &mut callback_source, &scheduler);

    assert_eq!(output, [0.0, 0.0]);
    assert!(matches!(
        scheduler.status(),
        crate::audio_priority::CallbackSchedulingStatus::Failed(failure)
            if failure.stage == SchedulingFailureStage::AffinityMismatch
    ));
    assert_eq!(
        guard.trace_for_cpu(0),
        vec![
            SchedulingSyscall::SetAffinity,
            SchedulingSyscall::GetAffinity
        ]
    );
}
