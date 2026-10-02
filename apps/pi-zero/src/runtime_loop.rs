use crate::host_adapter::PiPlaybackHostAdapter;
#[cfg(test)]
use playback_runtime::{CoreRunner, HostAdapter, RunnerMessage};
use playback_runtime::{HostMessage, NativeRunner, PlaybackRuntime};
use serde_json::Value;
use std::time::Instant;

pub(crate) use crate::runtime_output::process_runtime_output;

const PLATFORM_RESULT_BUDGET: usize = 4;

impl crate::runtime_output::PiRuntimeHost for PiPlaybackHostAdapter {
    const PREP_BOARD: crate::initial_audio_prep::InitialAudioPrepBoard =
        crate::initial_audio_prep::InitialAudioPrepBoard::Pi;

    fn dispatch(
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
        host: &mut Self,
        message: HostMessage,
    ) -> Result<(), String> {
        dispatch_runtime_message(playback, runner, host, message)
    }
    fn core(&self) -> &crate::pi_host_core::PiHostCore {
        &self.core
    }
    fn core_mut(&mut self) -> &mut crate::pi_host_core::PiHostCore {
        &mut self.core
    }
    fn shutdown_pending(&self) -> bool {
        PiPlaybackHostAdapter::shutdown_pending(self)
    }
    fn poll_recording_status(&self) -> Option<playback_runtime::RuntimeStoreResult> {
        PiPlaybackHostAdapter::poll_recording_status(self)
    }
    fn prep_audio_service(&self) -> crate::audio::AudioService {
        self.audio_service()
            .expect("initial Pi audio preparation requires an audio service")
    }
    fn drain_prep_host_results(&self, max_results: usize) -> Vec<HostMessage> {
        self.drain_platform_results(max_results)
    }
}

pub fn dispatch_runtime_message(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
    host_message: HostMessage,
) -> Result<(), String> {
    let output = playback.dispatch_host_message_music_first(host_message, runner, adapter)?;
    process_runtime_output(playback, runner, adapter, output)?;
    if let Some(message) = adapter.take_manual_save(playback, runner) {
        let output = playback.dispatch_host_message_music_first(message, runner, adapter)?;
        process_runtime_output(playback, runner, adapter, output)?;
    }
    Ok(())
}

pub fn report_runtime_failure(adapter: &PiPlaybackHostAdapter, prefix: &str, error: String) {
    if adapter.timing_evidence.is_some() {
        crate::timing_input::fail_study::<PiPlaybackHostAdapter>(error);
    }
    eprintln!("{prefix}: {error}");
}

pub fn handle_deferred_host_work(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
) -> Result<(), String> {
    if adapter.shutdown_pending() {
        return Ok(());
    }
    let responses = runner.poll_deferred_menu_apply_music_first()?;
    if !responses.is_empty() {
        let output = playback.dispatch_runner_messages(responses, runner, adapter)?;
        process_runtime_output(playback, runner, adapter, output)?;
    }
    if adapter.shutdown_pending() {
        return Ok(());
    }
    for result in adapter.flush_native_persistence_at(playback, runner, Instant::now()) {
        if let Some(evidence) = adapter.timing_evidence.as_mut() {
            evidence.record_host_message(&result);
        }
        dispatch_runtime_message(playback, runner, adapter, result)?;
    }
    for result in adapter.drain_platform_results_for_runner(runner, PLATFORM_RESULT_BUDGET) {
        if adapter.shutdown_pending() {
            break;
        }
        if let Some(evidence) = adapter.timing_evidence.as_mut() {
            evidence.record_host_message(&result);
        }
        dispatch_runtime_message(playback, runner, adapter, result)?;
    }
    Ok(())
}

pub fn latest_snapshot(playback: &PlaybackRuntime) -> Option<&Value> {
    playback.last_snapshot()
}

#[cfg(test)]
#[path = "runtime_loop_duck_slot_tests.rs"]
mod duck_slot_tests;
#[cfg(test)]
#[path = "runtime_loop_native_autosave_tests.rs"]
mod native_autosave_tests;

#[cfg(test)]
fn dispatch_and_ingest<R: CoreRunner, H: HostAdapter>(
    playback: &mut PlaybackRuntime,
    runner: &mut R,
    adapter: &mut H,
    host_message: HostMessage,
) -> Result<(), String> {
    playback
        .dispatch(
            playback_runtime::RuntimeDispatchInput::HostMessage(host_message),
            runner,
            adapter,
        )
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_output::ingest_oled_messages;
    use platform_core::MusicalEvent;
    use playback_runtime::RuntimeConfig;
    use playback_runtime::RuntimePlatformEffect;
    use serde_json::json;

    #[derive(Default)]
    struct FakeRunner;

    impl CoreRunner for FakeRunner {
        fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
            match message {
                HostMessage::DeviceInput { .. } => Ok(vec![RunnerMessage::AudioCommands {
                    commands: vec![playback_runtime::RuntimeAudioCommand::SetMasterVolume {
                        generation: 0,
                        volume_pct: 75.0,
                    }],
                }]),
                _ => Ok(Vec::new()),
            }
        }

        fn send_system_store_result(
            &mut self,
            _message: HostMessage,
        ) -> Result<
            (
                Vec<RunnerMessage>,
                Option<playback_runtime::RuntimeStoreResult>,
            ),
            String,
        > {
            Err("System store result not supported by fake runner".into())
        }
    }

    #[derive(Default)]
    struct CountingHostAdapter {
        audio_commands: usize,
    }

    impl HostAdapter for CountingHostAdapter {
        fn handle_musical_event(
            &mut self,
            _event: &MusicalEvent,
        ) -> Result<(), playback_runtime::RuntimeAdapterError> {
            Ok(())
        }

        fn handle_platform_effect(
            &mut self,
            _request: &playback_runtime::RuntimePlatformRequest,
        ) -> Result<Vec<HostMessage>, playback_runtime::RuntimeAdapterError> {
            Ok(Vec::new())
        }

        fn handle_audio_command(
            &mut self,
            _command: &playback_runtime::RuntimeAudioCommand,
        ) -> Result<(), playback_runtime::RuntimeAdapterError> {
            self.audio_commands += 1;
            Ok(())
        }

        fn handle_midi_message(
            &mut self,
            _bytes: &[u8],
        ) -> Result<(), playback_runtime::RuntimeAdapterError> {
            Ok(())
        }

        fn silence_internal_audio(&mut self) -> Result<(), playback_runtime::RuntimeAdapterError> {
            Ok(())
        }

        fn panic_external_midi(&mut self) -> Result<(), playback_runtime::RuntimeAdapterError> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct FollowUpRunner {
        runtime_results: usize,
    }

    impl CoreRunner for FollowUpRunner {
        fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
            match message {
                HostMessage::DeviceInput { .. } => Ok(vec![RunnerMessage::PlatformEffects {
                    effects: vec![RuntimePlatformEffect::StoreLoadDefault],
                }]),
                HostMessage::RuntimeResult { .. } => {
                    self.runtime_results += 1;
                    Ok(Vec::new())
                }
                _ => Ok(Vec::new()),
            }
        }

        fn send_system_store_result(
            &mut self,
            _message: HostMessage,
        ) -> Result<
            (
                Vec<RunnerMessage>,
                Option<playback_runtime::RuntimeStoreResult>,
            ),
            String,
        > {
            Err("System store result not supported by fake runner".into())
        }
    }

    #[derive(Default)]
    struct FollowUpHostAdapter;

    impl HostAdapter for FollowUpHostAdapter {
        fn handle_musical_event(
            &mut self,
            _event: &MusicalEvent,
        ) -> Result<(), playback_runtime::RuntimeAdapterError> {
            Ok(())
        }

        fn handle_platform_effect(
            &mut self,
            _request: &playback_runtime::RuntimePlatformRequest,
        ) -> Result<Vec<HostMessage>, playback_runtime::RuntimeAdapterError> {
            Ok(vec![HostMessage::RuntimeResult {
                result: playback_runtime::RuntimeStoreResult::LoadDefaultResult { payload: None },
            }])
        }

        fn handle_audio_command(
            &mut self,
            _command: &playback_runtime::RuntimeAudioCommand,
        ) -> Result<(), playback_runtime::RuntimeAdapterError> {
            Ok(())
        }

        fn handle_midi_message(
            &mut self,
            _bytes: &[u8],
        ) -> Result<(), playback_runtime::RuntimeAdapterError> {
            Ok(())
        }

        fn silence_internal_audio(&mut self) -> Result<(), playback_runtime::RuntimeAdapterError> {
            Ok(())
        }

        fn panic_external_midi(&mut self) -> Result<(), playback_runtime::RuntimeAdapterError> {
            Ok(())
        }
    }

    #[test]
    fn dispatch_ingests_runner_responses_once() {
        let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
        let mut runner = FakeRunner;
        let mut adapter = CountingHostAdapter::default();

        dispatch_and_ingest(
            &mut playback,
            &mut runner,
            &mut adapter,
            HostMessage::DeviceInput {
                input: json!({}),
                request_snapshot: None,
            },
        )
        .unwrap();

        assert_eq!(adapter.audio_commands, 1);
    }

    #[test]
    fn dispatch_processes_platform_effect_follow_ups() {
        let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
        let mut runner = FollowUpRunner::default();
        let mut adapter = FollowUpHostAdapter;

        dispatch_and_ingest(
            &mut playback,
            &mut runner,
            &mut adapter,
            HostMessage::DeviceInput {
                input: json!({}),
                request_snapshot: None,
            },
        )
        .unwrap();

        assert_eq!(runner.runtime_results, 1);
    }

    #[test]
    fn accepted_snapshot_ingestion_updates_raspberry_keyboard_gate() {
        let root = std::env::temp_dir().join(format!(
            "octessera-pi-keyboard-snapshot-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut adapter = PiPlaybackHostAdapter::new_with_data_role(
            None,
            root.join("store"),
            root.join("samples"),
            std::sync::Arc::new(|_| {}),
            false,
            playback_runtime::AudioOutputSet::jack(),
            playback_runtime::UsbDataRole::Host,
        );
        let control = crate::usb_keyboard::KeyboardCaptureControl::new(true);
        adapter.core.set_keyboard_capture_control(control.clone());
        ingest_oled_messages(
            &mut adapter,
            &[RunnerMessage::Snapshot {
                snapshot: json!({ "hdmi": { "mode": "live-grid" } }),
            }],
        );
        assert!(control.is_enabled());
        ingest_oled_messages(
            &mut adapter,
            &[RunnerMessage::Snapshot {
                snapshot: json!({ "hdmi": { "mode": "none" } }),
            }],
        );
        assert!(!control.is_enabled());
        let _ = std::fs::remove_dir_all(root);
    }
}
