use super::*;
use playback_runtime::{
    HostAdapter, MusicalEvent, RuntimeAdapterError, RuntimeAudioCommand, RuntimePlatformRequest,
};
use serde_json::json;

#[derive(Default)]
struct TestHost;

impl HostAdapter for TestHost {
    fn handle_musical_event(&mut self, _event: &MusicalEvent) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn handle_platform_effect(
        &mut self,
        _request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        Ok(Vec::new())
    }

    fn handle_audio_command(
        &mut self,
        _command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn handle_midi_message(&mut self, _bytes: &[u8]) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }
}

fn playing_playback() -> PlaybackRuntime {
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = NativeRunner::new(playback_runtime::NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    let mut host = TestHost;
    playback
        .dispatch_host_message(
            HostMessage::DeviceInput {
                input: json!({"type": "button_s", "pressed": true}),
                request_snapshot: Some(true),
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    playback
}

#[test]
fn active_xy_glide_keeps_stopped_playing_and_external_runtime_on_eight_millisecond_ticks() {
    let now = Instant::now();
    let stopped = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let external = PlaybackRuntime::new(playback_runtime::RuntimeConfig {
        sync_source: SyncSource::External,
        ..playback_runtime::RuntimeConfig::default()
    });
    let playing = playing_playback();
    let mut cases = vec![stopped, external, playing];

    for playback in &mut cases {
        let mut scheduler = HardwareRuntimeScheduler::new(now, 0);
        let deadline = Some(now + Duration::from_millis(10));
        assert!(scheduler
            .next_runtime_advance(now, playback, deadline)
            .is_none());
        assert!(scheduler
            .next_runtime_advance(
                now + PLAYBACK_TICK - Duration::from_nanos(1),
                playback,
                deadline,
            )
            .is_none());
        let advance = scheduler
            .next_runtime_advance(now + PLAYBACK_TICK, playback, deadline)
            .expect("active glide should receive the eight-millisecond tick");
        assert_eq!(advance.elapsed, PLAYBACK_TICK);
        assert!(!advance.request_snapshot);
        assert!(runtime_tick_needed(playback, deadline));
    }
}
