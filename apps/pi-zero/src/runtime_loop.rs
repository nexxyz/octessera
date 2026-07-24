use crate::host_adapter::PiPlaybackHostAdapter;
use playback_runtime::{
    CoreRunner, HostAdapter, HostMessage, NativeRunner, PlaybackRuntime, RunnerMessage,
    RuntimePlatformEffect,
};
use serde_json::Value;

const PLATFORM_RESULT_BUDGET: usize = 4;

pub fn dispatch_runtime_message(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
    host_message: HostMessage,
) -> Result<(), String> {
    let output = playback.dispatch(
        playback_runtime::RuntimeDispatchInput::HostMessage(host_message),
        runner,
        adapter,
    )?;
    for follow_up in output.follow_ups {
        dispatch_runtime_message(playback, runner, adapter, follow_up)?;
    }
    Ok(())
}

pub fn handle_deferred_host_work(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
) -> Result<(), String> {
    let responses = runner.flush_deferred_menu_apply()?;
    if !responses.is_empty() {
        ingest_responses(playback, runner, adapter, responses)?;
    }
    let follow_ups = adapter.flush_due_default_save()?;
    for follow_up in follow_ups {
        dispatch_runtime_message(playback, runner, adapter, follow_up)?;
    }
    for result in adapter.drain_platform_results(PLATFORM_RESULT_BUDGET) {
        dispatch_runtime_message(playback, runner, adapter, result)?;
    }
    Ok(())
}

pub fn initialize_host_state(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
) -> Result<(), String> {
    playback.dispatch_runner_messages(
        vec![playback_runtime::RunnerMessage::PlatformEffects {
            effects: vec![
                RuntimePlatformEffect::StoreLoadDefault,
                RuntimePlatformEffect::MidiListOutputsRequest,
                RuntimePlatformEffect::MidiListInputsRequest,
            ],
        }],
        runner,
        adapter,
    )?;
    Ok(())
}

pub fn latest_snapshot(playback: &PlaybackRuntime) -> Option<&Value> {
    playback.last_snapshot()
}

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

fn ingest_responses<R: CoreRunner, H: HostAdapter>(
    playback: &mut PlaybackRuntime,
    runner: &mut R,
    adapter: &mut H,
    responses: Vec<RunnerMessage>,
) -> Result<(), String> {
    playback
        .dispatch_runner_messages(responses, runner, adapter)
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_core::MusicalEvent;
    use playback_runtime::RuntimeConfig;
    use serde_json::json;

    #[derive(Default)]
    struct FakeRunner;

    impl CoreRunner for FakeRunner {
        fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
            match message {
                HostMessage::DeviceInput { .. } => Ok(vec![RunnerMessage::AudioCommands {
                    commands: vec![playback_runtime::RuntimeAudioCommand::SetMasterVolume {
                        volume_pct: 75.0,
                    }],
                }]),
                _ => Ok(Vec::new()),
            }
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
}
