use crate::host_adapter::PiPlaybackHostAdapter;
use playback_runtime::{
    CoreRunner, HostAdapter, HostMessage, NativeRunner, PlaybackRuntime, RunnerMessage,
    RuntimeConfig, RuntimePlatformEffect, SyncSource,
};
use serde_json::Value;
use std::collections::VecDeque;

const PLATFORM_RESULT_BUDGET: usize = 4;

pub fn dispatch_runtime_message(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
    host_message: HostMessage,
) -> Result<(), String> {
    dispatch_and_ingest(playback, runner, adapter, host_message)
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
    for effect in [
        RuntimePlatformEffect::StoreLoadDefault,
        RuntimePlatformEffect::MidiListOutputsRequest,
        RuntimePlatformEffect::MidiListInputsRequest,
    ] {
        for follow_up in adapter.handle_platform_effect(&effect)? {
            dispatch_runtime_message(playback, runner, adapter, follow_up)?;
        }
    }
    Ok(())
}

pub fn latest_snapshot(playback: &PlaybackRuntime) -> Option<&Value> {
    playback.last_snapshot()
}

pub fn sync_playback_config_from_snapshot(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    snapshot: &Value,
) {
    let Some(config) = playback_config_from_snapshot(snapshot) else {
        return;
    };
    if playback.config() == &config {
        return;
    }
    playback.set_config(config);
    runner.apply_runtime_config(playback.config());
}

pub fn playback_config_matches_snapshot(playback: &PlaybackRuntime, snapshot: &Value) -> bool {
    playback_config_from_snapshot(snapshot).is_none_or(|config| playback.config() == &config)
}

fn playback_config_from_snapshot(snapshot: &Value) -> Option<RuntimeConfig> {
    let midi = snapshot.get("settings")?.get("midi")?;
    let transport = snapshot.get("transport").unwrap_or(&Value::Null);
    let sync_source = match midi.get("syncMode").and_then(Value::as_str) {
        Some("external") => SyncSource::External,
        _ => SyncSource::Internal,
    };
    let midi_enabled = midi
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let midi_out_selected = midi.get("outId").is_some_and(|value| !value.is_null());
    Some(RuntimeConfig {
        bpm: transport
            .get("bpm")
            .and_then(Value::as_f64)
            .unwrap_or(120.0),
        sync_source,
        midi_clock_out_enabled: midi
            .get("clockOutEnabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        midi_out_enabled: midi_enabled && midi_out_selected,
    })
}

fn dispatch_and_ingest<R: CoreRunner, H: HostAdapter>(
    playback: &mut PlaybackRuntime,
    runner: &mut R,
    adapter: &mut H,
    host_message: HostMessage,
) -> Result<(), String> {
    let mut queue = VecDeque::from([host_message]);
    while let Some(message) = queue.pop_front() {
        crate::wake_trace::log_host_dispatch(&message);
        let responses = runner.send(message)?;
        for follow_up in playback.ingest_runner_messages(responses, adapter)? {
            queue.push_back(follow_up);
        }
    }
    Ok(())
}

fn ingest_responses<R: CoreRunner, H: HostAdapter>(
    playback: &mut PlaybackRuntime,
    runner: &mut R,
    adapter: &mut H,
    responses: Vec<RunnerMessage>,
) -> Result<(), String> {
    let mut queue = VecDeque::from(playback.ingest_runner_messages(responses, adapter)?);
    while let Some(message) = queue.pop_front() {
        dispatch_and_ingest(playback, runner, adapter, message)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_core::MusicalEvent;
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
        fn handle_musical_event(&mut self, _event: &MusicalEvent) -> Result<(), String> {
            Ok(())
        }

        fn handle_platform_effect(
            &mut self,
            _effect: &RuntimePlatformEffect,
        ) -> Result<Vec<HostMessage>, String> {
            Ok(Vec::new())
        }

        fn handle_audio_command(
            &mut self,
            _command: &playback_runtime::RuntimeAudioCommand,
        ) -> Result<(), String> {
            self.audio_commands += 1;
            Ok(())
        }

        fn handle_midi_message(&mut self, _bytes: &[u8]) -> Result<(), String> {
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
        fn handle_musical_event(&mut self, _event: &MusicalEvent) -> Result<(), String> {
            Ok(())
        }

        fn handle_platform_effect(
            &mut self,
            _effect: &RuntimePlatformEffect,
        ) -> Result<Vec<HostMessage>, String> {
            Ok(vec![HostMessage::RuntimeResult {
                result: playback_runtime::RuntimeStoreResult::LoadDefaultResult { payload: None },
            }])
        }

        fn handle_audio_command(
            &mut self,
            _command: &playback_runtime::RuntimeAudioCommand,
        ) -> Result<(), String> {
            Ok(())
        }

        fn handle_midi_message(&mut self, _bytes: &[u8]) -> Result<(), String> {
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
    fn playback_config_from_snapshot_tracks_midi_runtime_settings() {
        let snapshot = json!({
            "transport": { "bpm": 93.5 },
            "settings": {
                "midi": {
                    "enabled": true,
                    "outId": "0",
                    "syncMode": "external",
                    "clockOutEnabled": true
                }
            }
        });

        let config = playback_config_from_snapshot(&snapshot).unwrap();

        assert_eq!(config.bpm, 93.5);
        assert_eq!(config.sync_source, SyncSource::External);
        assert!(config.midi_clock_out_enabled);
        assert!(config.midi_out_enabled);
    }
}
