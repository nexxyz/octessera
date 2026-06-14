mod native_help;
mod native_menu;
mod native_runner;
mod process_runner;
mod protocol;
mod runtime;

pub use native_runner::{NativeRunner, NativeRunnerConfig};
pub use platform_core::MusicalEvent;
pub use process_runner::{workspace_root_from, NodeRunnerProcess};
pub use protocol::{
    HostMessage, MidiPort, RunnerMessage, RuntimeAudioCommand, RuntimeMomentaryFxTarget,
    RuntimePlatformEffect, RuntimeStatus, RuntimeStatusState, RuntimeStoreResult,
    RuntimeTransportState, SampleEntry, SyncSource,
};
pub use runtime::{CoreRunner, HostAdapter, PlaybackRuntime, RuntimeConfig};

#[cfg(test)]
mod tests {
    use super::{
        workspace_root_from, CoreRunner, HostAdapter, HostMessage, MusicalEvent, NativeRunner,
        NativeRunnerConfig, NodeRunnerProcess, PlaybackRuntime, RunnerMessage, RuntimeAudioCommand,
        RuntimeConfig, RuntimePlatformEffect, RuntimeStatus, RuntimeStatusState,
        RuntimeStoreResult, RuntimeTransportState, SyncSource,
    };
    use serde_json::json;
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use tempfile::tempdir;

    #[derive(Default)]
    struct FakeRunner {
        seen: Vec<HostMessage>,
    }

    impl CoreRunner for FakeRunner {
        fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
            self.seen.push(message.clone());
            match message {
                HostMessage::TransportPulseStep { pulses, .. } => Ok(vec![
                    RunnerMessage::MusicalEvents {
                        events: vec![MusicalEvent::NoteOn {
                            channel: 0,
                            note: 60,
                            velocity: 100,
                            duration_ms: Some(30),
                        }],
                    },
                    RunnerMessage::RuntimeStatus {
                        status: RuntimeStatus {
                            state: RuntimeStatusState::Running,
                            transport: RuntimeTransportState::Playing,
                            current_ppqn_pulse: pulses as u64,
                            pending_resync: false,
                            sync_source: SyncSource::Internal,
                            message: None,
                        },
                    },
                ]),
                HostMessage::MidiRealtimeStart => Ok(vec![RunnerMessage::RuntimeStatus {
                    status: RuntimeStatus {
                        state: RuntimeStatusState::Running,
                        transport: RuntimeTransportState::Playing,
                        current_ppqn_pulse: 0,
                        pending_resync: false,
                        sync_source: SyncSource::External,
                        message: None,
                    },
                }]),
                HostMessage::MidiRealtimeClock { pulses } => {
                    Ok(vec![RunnerMessage::RuntimeStatus {
                        status: RuntimeStatus {
                            state: RuntimeStatusState::Running,
                            transport: RuntimeTransportState::Playing,
                            current_ppqn_pulse: pulses as u64,
                            pending_resync: false,
                            sync_source: SyncSource::External,
                            message: None,
                        },
                    }])
                }
                HostMessage::MidiRealtimeStop => Ok(vec![RunnerMessage::RuntimeStatus {
                    status: RuntimeStatus {
                        state: RuntimeStatusState::Paused,
                        transport: RuntimeTransportState::Stopped,
                        current_ppqn_pulse: 2,
                        pending_resync: false,
                        sync_source: SyncSource::External,
                        message: None,
                    },
                }]),
                HostMessage::RuntimeResult { result } => Ok(vec![
                    RunnerMessage::Snapshot {
                        snapshot: json!({ "result": result }),
                    },
                    RunnerMessage::RuntimeStatus {
                        status: RuntimeStatus {
                            state: RuntimeStatusState::Idle,
                            transport: RuntimeTransportState::Stopped,
                            current_ppqn_pulse: 0,
                            pending_resync: false,
                            sync_source: SyncSource::Internal,
                            message: None,
                        },
                    },
                ]),
                HostMessage::DeviceInput { .. } | HostMessage::MidiRealtimeContinue => Ok(vec![]),
            }
        }
    }

    #[derive(Default)]
    struct FakeHost {
        midi_messages: Vec<Vec<u8>>,
        audio_commands: Vec<RuntimeAudioCommand>,
        effects: Vec<RuntimePlatformEffect>,
    }

    impl HostAdapter for FakeHost {
        fn handle_musical_event(&mut self, _event: &MusicalEvent) -> Result<(), String> {
            Ok(())
        }

        fn handle_platform_effect(
            &mut self,
            effect: &RuntimePlatformEffect,
        ) -> Result<Vec<HostMessage>, String> {
            self.effects.push(effect.clone());
            if matches!(effect, RuntimePlatformEffect::StoreListPresets) {
                return Ok(vec![HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::ListPresetsResult {
                        names: vec!["Factory".into(), "Live Set".into()],
                    },
                }]);
            }
            Ok(vec![])
        }

        fn handle_audio_command(&mut self, command: &RuntimeAudioCommand) -> Result<(), String> {
            self.audio_commands.push(command.clone());
            Ok(())
        }

        fn handle_midi_message(&mut self, bytes: &[u8]) -> Result<(), String> {
            self.midi_messages.push(bytes.to_vec());
            Ok(())
        }
    }

    #[test]
    fn internal_clock_advances_runner_and_schedules_note_offs() {
        let mut runtime = PlaybackRuntime::new(RuntimeConfig {
            bpm: 120.0,
            sync_source: SyncSource::Internal,
            midi_clock_out_enabled: true,
            midi_out_enabled: true,
        });
        let mut runner = FakeRunner::default();
        let mut host = FakeHost::default();

        runtime.advance(500, &mut runner, &mut host).unwrap();

        assert!(matches!(
            runner.seen[0],
            HostMessage::TransportPulseStep { pulses: 24, .. }
        ));
        assert_eq!(host.midi_messages[0], vec![0x90, 60, 100]);
        assert_eq!(host.midi_messages[1], vec![0xFA]);
        assert_eq!(host.midi_messages.len(), 26);

        runtime.advance(30, &mut runner, &mut host).unwrap();
        assert!(host.midi_messages.contains(&vec![0x80, 60, 0]));
    }

    #[test]
    fn external_midi_realtime_bytes_are_aggregated_into_runner_messages() {
        let mut runtime = PlaybackRuntime::new(RuntimeConfig {
            bpm: 120.0,
            sync_source: SyncSource::External,
            midi_clock_out_enabled: true,
            midi_out_enabled: true,
        });
        let mut runner = FakeRunner::default();
        let mut host = FakeHost::default();

        runtime
            .handle_midi_realtime_bytes(&[0xFA, 0xF8, 0xF8, 0xFC], &mut runner, &mut host)
            .unwrap();

        assert_eq!(runner.seen.len(), 3);
        assert!(matches!(runner.seen[0], HostMessage::MidiRealtimeStart));
        assert!(matches!(runner.seen[1], HostMessage::MidiRealtimeStop));
        assert!(matches!(
            runner.seen[2],
            HostMessage::MidiRealtimeClock { pulses: 2 }
        ));
        assert!(host.midi_messages.is_empty());
    }

    #[test]
    fn host_effect_results_round_trip_back_into_runner() {
        struct EffectRunner;

        impl CoreRunner for EffectRunner {
            fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
                match message {
                    HostMessage::TransportPulseStep { .. } => {
                        Ok(vec![RunnerMessage::PlatformEffects {
                            effects: vec![RuntimePlatformEffect::StoreListPresets],
                        }])
                    }
                    HostMessage::RuntimeResult { result } => Ok(vec![RunnerMessage::Snapshot {
                        snapshot: json!({ "roundTrip": result }),
                    }]),
                    _ => Ok(vec![]),
                }
            }
        }

        let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
        let mut runner = EffectRunner;
        let mut host = FakeHost::default();

        runtime.advance(500, &mut runner, &mut host).unwrap();

        assert_eq!(host.effects, vec![RuntimePlatformEffect::StoreListPresets]);
        assert_eq!(
            runtime.last_snapshot(),
            Some(&json!({
                "roundTrip": {
                    "type": "list_presets_result",
                    "names": ["Factory", "Live Set"]
                }
            }))
        );
    }

    #[test]
    fn native_runner_rejects_unsupported_behavior() {
        let error = NativeRunner::new(NativeRunnerConfig {
            behavior_id: "unsupported".into(),
            ..NativeRunnerConfig::default()
        })
        .err()
        .unwrap();
        assert!(error.contains("unsupported native behavior `unsupported`"));
    }

    #[test]
    fn native_runner_transport_tick_returns_status_and_snapshot() {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        let _ = runner.send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
        });
        let messages = runner
            .send(HostMessage::TransportPulseStep {
                pulses: 24,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(true),
            })
            .unwrap();
        assert!(matches!(
            messages.last(),
            Some(RunnerMessage::RuntimeStatus { .. })
        ));
        assert!(messages
            .iter()
            .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
    }

    #[test]
    fn workspace_root_resolves_repo_from_crate_dir() {
        let root = workspace_root_from(Path::new(env!("CARGO_MANIFEST_DIR")));
        assert!(root.join("pnpm-workspace.yaml").is_file());
        assert!(root.join("packages").is_dir());
    }

    #[test]
    fn workspace_root_resolves_repo_from_desktop_tauri_dir() {
        let root = workspace_root_from(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("apps")
                .join("desktop")
                .join("src-tauri"),
        );
        assert!(root.join("pnpm-workspace.yaml").is_file());
    }

    #[test]
    fn process_runner_round_trips_json_lines() {
        let dir = tempdir().unwrap();
        let runner_path = dir.path().join("runner.js");
        fs::write(
            &runner_path,
            r#"
const readline = require('node:readline');
const rl = readline.createInterface({ input: process.stdin });
rl.on('line', (line) => {
  const msg = JSON.parse(line);
  if (msg.type === 'transport_pulse_step') {
    process.stdout.write(JSON.stringify({ type: 'runtime_status', status: { state: 'running', transport: 'playing', currentPpqnPulse: msg.pulses, pendingResync: false, syncSource: 'internal', message: null } }) + '\n');
  }
});
"#,
        )
        .unwrap();

        let mut command = Command::new("node");
        command.arg(&runner_path).current_dir(dir.path());
        let mut runner = NodeRunnerProcess::spawn(command).unwrap();
        let responses = runner
            .send(HostMessage::TransportPulseStep {
                pulses: 7,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: None,
            })
            .unwrap();
        assert_eq!(responses.len(), 1);
        assert!(matches!(
            &responses[0],
            RunnerMessage::RuntimeStatus { status }
                if status.current_ppqn_pulse == 7
                    && status.transport == RuntimeTransportState::Playing
        ));
    }

    #[test]
    fn panic_clears_pending_notes_and_sends_all_notes_off() {
        let mut runtime = PlaybackRuntime::new(RuntimeConfig {
            bpm: 120.0,
            sync_source: SyncSource::Internal,
            midi_clock_out_enabled: false,
            midi_out_enabled: true,
        });
        let mut runner = FakeRunner::default();
        let mut host = FakeHost::default();

        runtime.advance(500, &mut runner, &mut host).unwrap();
        host.midi_messages.clear();

        runtime.panic(&mut host).unwrap();

        assert_eq!(host.midi_messages.first(), Some(&vec![0xFC]));
        assert_eq!(host.midi_messages.len(), 33);
    }
}
