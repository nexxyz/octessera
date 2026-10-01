use super::{CoreRunner, HostAdapter, PlaybackRuntime, RuntimeIngest, RuntimeOperation};
use crate::{HostMessage, NativeRunner, RunnerMessage, RuntimePlatformRequest};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeStoreRequest {
    operation: RuntimeOperation,
    request_id: String,
    revision: u64,
}

impl NativeStoreRequest {
    pub fn operation(&self) -> &RuntimeOperation {
        &self.operation
    }

    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }
}

impl PlaybackRuntime {
    pub fn next_native_store_request(
        &mut self,
        operation: RuntimeOperation,
        revision: u64,
    ) -> NativeStoreRequest {
        self.next_request_id = self.next_request_id.saturating_add(1);
        let request_id = if operation == RuntimeOperation::StoreSavePreset {
            format!("native-preset-{}", self.next_request_id)
        } else {
            format!("platform-{}", self.next_request_id)
        };
        NativeStoreRequest {
            operation,
            request_id,
            revision,
        }
    }
}

struct MusicFirstRunner<'a>(&'a mut NativeRunner);

impl CoreRunner for MusicFirstRunner<'_> {
    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        self.0.send_music_first(message)
    }

    fn register_platform_request(&mut self, request: &RuntimePlatformRequest) {
        self.0.register_platform_request(request);
    }

    fn send_system_store_result(
        &mut self,
        message: HostMessage,
    ) -> Result<(Vec<RunnerMessage>, Option<crate::RuntimeStoreResult>), String> {
        <NativeRunner as CoreRunner>::send_system_store_result(self.0, message)
    }

    fn send_store_result_handoff(
        &mut self,
        message: HostMessage,
    ) -> Result<(Vec<RunnerMessage>, Option<crate::RuntimeStoreResult>, bool), String> {
        <NativeRunner as CoreRunner>::send_store_result_handoff(self.0, message)
    }
}

impl PlaybackRuntime {
    pub fn dispatch_host_message_music_first<H: HostAdapter>(
        &mut self,
        message: HostMessage,
        runner: &mut NativeRunner,
        host: &mut H,
    ) -> Result<RuntimeIngest, String> {
        self.dispatch_host_message(message, &mut MusicFirstRunner(runner), host)
    }

    pub fn advance_duration_music_first_with_output<H: HostAdapter>(
        &mut self,
        elapsed: Duration,
        runner: &mut NativeRunner,
        host: &mut H,
    ) -> Result<RuntimeIngest, String> {
        self.advance_duration_with_output(elapsed, &mut MusicFirstRunner(runner), host)
    }

    pub fn handle_midi_realtime_bytes_music_first_with_output<H: HostAdapter>(
        &mut self,
        bytes: &[u8],
        runner: &mut NativeRunner,
        host: &mut H,
    ) -> Result<RuntimeIngest, String> {
        self.handle_midi_realtime_bytes_with_output(bytes, &mut MusicFirstRunner(runner), host)
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::support::FakeHost;
    use crate::{
        DrumHit, HostAdapter, HostMessage, MusicalEvent, NativeRunner, NativeRunnerConfig,
        PlaybackRuntime, RunnerMessage, RuntimeAdapterError, RuntimeAudioCommand, RuntimeConfig,
        RuntimeOperation, RuntimePlatformEffect, RuntimePlatformRequest, RuntimeTransportState,
    };
    use serde_json::{json, Value};
    use std::time::Duration;

    #[test]
    fn native_store_request_metadata_shares_the_platform_request_id_sequence() {
        let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
        let native = runtime.next_native_store_request(RuntimeOperation::StoreSaveDefault, 7);
        let preset = runtime.next_native_store_request(RuntimeOperation::StoreSavePreset, 8);
        let regular = runtime.next_platform_request(RuntimePlatformEffect::StoreSaveBackup {
            payload: json!({ "revision": 9 }),
        });

        assert_eq!(native.operation(), &RuntimeOperation::StoreSaveDefault);
        assert_eq!(native.revision(), 7);
        assert_eq!(native.request_id(), "platform-1");
        assert_eq!(preset.operation(), &RuntimeOperation::StoreSavePreset);
        assert_eq!(preset.revision(), 8);
        assert_eq!(preset.request_id(), "native-preset-2");
        assert_eq!(regular.request_id, "platform-3");
        assert_eq!(regular.revision, Some(9));
    }

    #[derive(Default)]
    struct RecordingHost {
        inner: FakeHost,
        calls: Vec<&'static str>,
    }

    impl HostAdapter for RecordingHost {
        fn handle_musical_event(
            &mut self,
            event: &MusicalEvent,
        ) -> Result<(), RuntimeAdapterError> {
            self.calls.push("note");
            self.inner.handle_musical_event(event)
        }

        fn handle_drum_hit(&mut self, hit: &DrumHit) -> Result<(), RuntimeAdapterError> {
            self.calls.push("drum");
            self.inner.handle_drum_hit(hit)
        }

        fn handle_audio_command(
            &mut self,
            command: &RuntimeAudioCommand,
        ) -> Result<(), RuntimeAdapterError> {
            self.calls.push(
                if matches!(command, RuntimeAudioCommand::SetAudioConfig { .. }) {
                    "config"
                } else {
                    "audio"
                },
            );
            self.inner.handle_audio_command(command)
        }

        fn handle_platform_effect(
            &mut self,
            request: &RuntimePlatformRequest,
        ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
            self.inner.handle_platform_effect(request)
        }

        fn handle_midi_message(&mut self, bytes: &[u8]) -> Result<(), RuntimeAdapterError> {
            self.inner.handle_midi_message(bytes)
        }

        fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError> {
            self.inner.silence_internal_audio()
        }

        fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
            self.inner.panic_external_midi()
        }
    }

    #[test]
    fn runtime_music_first_delivers_host_events_before_scene_capture() {
        let payload: Value =
            serde_json::from_str(include_str!("../../../config/generated/pi/default.json"))
                .unwrap();
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.apply_config_payload(payload).unwrap();
        runner.skip_startup_splash();
        let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
        let mut host = RecordingHost::default();
        let startup = runner.messages_with_snapshot().unwrap();
        runtime
            .dispatch_runner_messages(startup, &mut runner, &mut host)
            .unwrap();
        for pressed in [true, false] {
            runtime
                .dispatch_host_message_music_first(
                    HostMessage::DeviceInput {
                        input: json!({"type": "button_s", "pressed": pressed}),
                        request_snapshot: None,
                    },
                    &mut runner,
                    &mut host,
                )
                .unwrap();
        }
        assert_eq!(
            runtime.last_status().unwrap().transport,
            RuntimeTransportState::Playing
        );
        host.calls.clear();
        host.inner.audio_commands.clear();
        runner.test_bump_audio_config_revision();
        let before = host.inner.musical_events.len();
        let output = runtime
            .advance_duration_music_first_with_output(
                Duration::from_millis(500),
                &mut runner,
                &mut host,
            )
            .unwrap();
        assert!(host.inner.musical_events.len() > before);
        let config = host
            .calls
            .iter()
            .position(|call| *call == "config")
            .unwrap();
        let note = host
            .calls
            .iter()
            .position(|call| *call == "note" || *call == "drum")
            .unwrap();
        assert!(config < note);
        assert!(host
            .inner
            .audio_commands
            .iter()
            .any(|command| matches!(command, RuntimeAudioCommand::SetAudioConfig { .. })));
        assert!(output
            .messages
            .iter()
            .all(|message| !matches!(message, RunnerMessage::Snapshot { .. })));
        let _scene = runner.capture_display_scene().unwrap();
        assert_eq!(
            runtime.last_status().unwrap().transport,
            RuntimeTransportState::Playing
        );
        for _ in 0..4 {
            let next = runtime
                .advance_duration_music_first_with_output(
                    Duration::from_millis(8),
                    &mut runner,
                    &mut host,
                )
                .unwrap();
            assert!(next
                .messages
                .iter()
                .all(|message| !matches!(message, RunnerMessage::Snapshot { .. })));
        }
        assert!(runner.display_scene_pending());
    }
}
