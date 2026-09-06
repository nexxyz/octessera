use super::audio_output_open::{source_execution_mode, AudioConstructionConfig};
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use super::cpal_audio_output::OrangeAudioProfile;
use super::cpal_audio_output::{build_engine_source, AudioSourceExecutionMode};
use super::AudioSink;
use crate::audio_priority::{
    callback_priority, ORANGE_CALLBACK_PRIORITY, ORANGE_SECONDARY_CALLBACK_PRIORITY,
    ORANGE_WORKER_PRIORITY, RASPBERRY_CALLBACK_PRIORITY,
};
use realtime_engine::synth::SourceWorkerHealth;
use rodio_engine_source::event_queue;

#[test]
fn inline_cpal_mode_does_not_start_workers_before_stream_construction() {
    let (_engine_tx, engine_rx) = event_queue();
    let (source, shutdown_owner) = build_engine_source(
        engine_rx,
        48_000,
        AudioSourceExecutionMode::Inline,
        32,
        None,
        [None, None],
    )
    .unwrap();

    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Disabled);
    drop(source);
    assert!(shutdown_owner.is_none());
}

#[test]
fn cpal_uses_routing_tree_only_for_orange_capacity_jack() {
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    let config = AudioConstructionConfig::orange(OrangeAudioProfile::from_optimization(
        playback_runtime::AudioOptimization::Capacity,
    ));
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    let config = AudioConstructionConfig::raspberry(None);
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    assert_eq!(
        source_execution_mode(AudioSink::Jack, config),
        AudioSourceExecutionMode::RoutingTree
    );
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    assert_eq!(
        source_execution_mode(AudioSink::Jack, config),
        AudioSourceExecutionMode::Inline
    );
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    assert_eq!(
        source_execution_mode(
            AudioSink::Jack,
            AudioConstructionConfig::orange(OrangeAudioProfile::from_optimization(
                playback_runtime::AudioOptimization::Latency,
            )),
        ),
        AudioSourceExecutionMode::Inline
    );
    for sink in [AudioSink::Usb, AudioSink::Hdmi] {
        assert_eq!(
            source_execution_mode(sink, config),
            AudioSourceExecutionMode::Inline
        );
    }
}

#[test]
fn cpal_worker_status_keeps_deadline_miss_nonterminal() {
    for health in [SourceWorkerHealth::Disabled, SourceWorkerHealth::Healthy] {
        assert!(!health.is_terminal());
    }
    assert!(!SourceWorkerHealth::DeadlineMiss.is_terminal());
    for health in [
        SourceWorkerHealth::DispatchFailed,
        SourceWorkerHealth::CompletionFailed,
        SourceWorkerHealth::WorkerExited,
        SourceWorkerHealth::InvalidBlock,
    ] {
        assert!(health.is_terminal());
    }
}

#[test]
fn cpal_qualification_priorities_keep_orange_workers_above_callbacks() {
    assert_eq!(ORANGE_WORKER_PRIORITY, 70);
    assert_eq!(ORANGE_CALLBACK_PRIORITY, 70);
    assert_eq!(ORANGE_SECONDARY_CALLBACK_PRIORITY, 69);
    assert_eq!(RASPBERRY_CALLBACK_PRIORITY, 70);
    const {
        assert!(ORANGE_CALLBACK_PRIORITY <= ORANGE_WORKER_PRIORITY);
    }
    assert_eq!(
        callback_priority(),
        if cfg!(feature = "hardware-orange-pi-zero-2w") {
            ORANGE_SECONDARY_CALLBACK_PRIORITY
        } else {
            RASPBERRY_CALLBACK_PRIORITY
        }
    );
}

#[test]
fn only_orange_jack_uses_strict_callback_placement() {
    for sink in [AudioSink::Usb, AudioSink::Hdmi] {
        assert!(!super::cpal_audio_output::callback_scheduler_for_sink(sink).is_strict());
    }
    assert_eq!(
        super::cpal_audio_output::callback_scheduler_for_sink(AudioSink::Jack).is_strict(),
        cfg!(feature = "hardware-orange-pi-zero-2w")
    );
}
