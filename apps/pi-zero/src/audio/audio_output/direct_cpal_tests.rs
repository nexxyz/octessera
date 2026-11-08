use super::audio_output_open::{source_execution_mode, AudioConstructionConfig};
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use super::audio_profile::OrangeAudioProfile;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use super::audio_profile::RaspberryAudioProfile;
use super::cpal_audio_output::{
    build_engine_source, AudioSourceExecutionMode, EngineSourceOptions, StreamBuildOptions,
};
use super::AudioSink;
use crate::audio_priority::{
    ORANGE_WORKER_PRIORITY, PI_JACK_CALLBACK_PRIORITY, PI_MIRROR_CALLBACK_PRIORITY,
};
use crate::audio_stream_health::AudioStreamHealth;
use realtime_engine::synth::{prepare_instruments_config, SourceWorkerHealth};
use rodio_engine_source::{event_queue, AudioLoadStatusSender, EngineEvent};
use std::time::Duration;

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
fn latency_jack_source_reports_output_clipping_on_the_load_status_channel() {
    let (load_tx, load_rx) = rodio_engine_source::audio_load_status_channel();
    play_clipping_latency_jack_source(Some(load_tx));

    let statuses: Vec<_> = std::iter::from_fn(|| load_rx.try_recv().ok()).collect();
    assert!(
        !statuses.is_empty(),
        "Latency Jack source sent no load status"
    );
    assert!(
        statuses.iter().any(|status| status.output_clipped),
        "no load status reported the clipped output"
    );
    for status in statuses {
        assert_eq!(status.worker_utilization, None);
        assert!(!status.missed_quantum_flash);
    }
}

pub(super) fn latency_config() -> AudioConstructionConfig {
    let latency = playback_runtime::AudioOptimization::Latency;
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    let config = AudioConstructionConfig::orange(OrangeAudioProfile::from_optimization(latency));
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    let config =
        AudioConstructionConfig::raspberry(RaspberryAudioProfile::from_optimization(latency));
    config
}

pub(super) fn play_clipping_latency_jack_source(load_tx: Option<AudioLoadStatusSender>) {
    let options = StreamBuildOptions::new(
        AudioSink::Jack,
        source_execution_mode(AudioSink::Jack, latency_config()),
        EngineSourceOptions {
            mix_taps: None,
            load_tx,
            mirror_producers: [None, None],
        },
        AudioStreamHealth::new("Jack".into()),
    );
    assert_eq!(options.execution_mode, AudioSourceExecutionMode::Inline);
    assert!(
        options.load_tx.is_some(),
        "Latency Jack stream options dropped the load-status sender"
    );
    let (engine_tx, engine_rx) = event_queue();
    let (mut source, _) = build_engine_source(
        engine_rx,
        48_000,
        options.execution_mode,
        32,
        options.load_tx,
        options.mirror_producers,
    )
    .unwrap();
    engine_tx
        .send(EngineEvent::SetPreparedInstruments {
            generation: 0,
            config: prepare_instruments_config(crate::audio::default_pi_instruments(), 48_000),
        })
        .unwrap();
    for (instrument_slot, note) in (0..4).flat_map(|slot| (48..56).map(move |note| (slot, note))) {
        engine_tx
            .send(EngineEvent::NoteOn {
                instrument_slot,
                note,
                velocity: 127,
                duration_ms: 2_000,
                at: None,
            })
            .unwrap();
    }
    let mut render_frames = |frames: usize| {
        for _ in 0..frames * 2 {
            source.next();
        }
    };
    render_frames(12_000);
    std::thread::sleep(Duration::from_millis(150));
    render_frames(32);
}

#[test]
fn cpal_uses_routing_tree_only_for_capacity_jack() {
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    let config = AudioConstructionConfig::orange(OrangeAudioProfile::from_optimization(
        playback_runtime::AudioOptimization::Capacity,
    ));
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    let config = AudioConstructionConfig::raspberry(RaspberryAudioProfile::from_optimization(
        playback_runtime::AudioOptimization::Latency,
    ));
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

#[cfg(feature = "hardware-raspberry-pi-zero-2w")]
#[test]
fn raspberry_capacity_uses_routing_tree_only_for_jack() {
    let config = AudioConstructionConfig::raspberry(RaspberryAudioProfile::from_optimization(
        playback_runtime::AudioOptimization::Capacity,
    ));

    assert_eq!(
        source_execution_mode(AudioSink::Jack, config),
        AudioSourceExecutionMode::RoutingTree
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
    assert_eq!(PI_JACK_CALLBACK_PRIORITY, 70);
    assert_eq!(PI_MIRROR_CALLBACK_PRIORITY, 60);
    const {
        assert!(PI_JACK_CALLBACK_PRIORITY <= ORANGE_WORKER_PRIORITY);
    }
}

#[test]
fn all_pi_callbacks_use_strict_fixed_roles() {
    let jack = super::cpal_audio_output::callback_scheduler_for_sink(AudioSink::Jack);
    assert_eq!(jack.requested_cpu(), 1);
    assert_eq!(jack.requested_priority(), 70);
    for sink in [AudioSink::Usb, AudioSink::Hdmi] {
        let mirror = super::cpal_audio_output::callback_scheduler_for_sink(sink);
        assert_eq!(mirror.requested_cpu(), 0);
        assert_eq!(mirror.requested_priority(), 60);
    }
}
