#![recursion_limit = "256"]

mod delay_timing;
mod native_help;
mod native_menu;
mod native_runner;
mod preset_name_policy;
mod protocol;
mod runtime;
mod timing_probe;
mod timing_units;

pub use native_runner::{NativeRunner, NativeRunnerConfig};
pub use platform_core::MusicalEvent;
pub use preset_name_policy::{clean_preset_name, fresh_preset_name, is_valid_preset_name};
pub use protocol::{
    HostMessage, MidiPort, RunnerMessage, RuntimeAdapterError, RuntimeAudioCommand,
    RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeErrorMetadata,
    RuntimeMomentaryFxTarget, RuntimeOperation, RuntimePlatformEffect, RuntimePlatformRequest,
    RuntimeRecovery, RuntimeStatus, RuntimeStatusState, RuntimeStoreResult, RuntimeTransportState,
    RuntimeUiPulse, SampleEntry, SyncSource,
};
pub use runtime::{
    CoreRunner, HostAdapter, PlaybackRuntime, RuntimeConfig, RuntimeDispatchInput, RuntimeIngest,
};
pub use timing_probe::{
    parse_timing_probe_durations, parse_timing_probe_scenarios, print_timing_probe_summary,
    run_timing_probe, TimingProbeOptions, TimingProbeReport, TimingProbeScenario,
};

#[cfg(test)]
mod tests {
    use super::{
        CoreRunner, HostAdapter, HostMessage, MusicalEvent, NativeRunner, NativeRunnerConfig,
        PlaybackRuntime, RunnerMessage, RuntimeAdapterError, RuntimeAudioCommand, RuntimeConfig,
        RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeErrorMetadata,
        RuntimeOperation, RuntimePlatformEffect, RuntimePlatformRequest, RuntimeRecovery,
        RuntimeStatus, RuntimeStatusState, RuntimeStoreResult, RuntimeTransportState, SyncSource,
    };
    use serde_json::json;

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
                            error: None,
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
                        error: None,
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
                            error: None,
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
                        error: None,
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
                            error: None,
                        },
                    },
                ]),
                HostMessage::TransportStop => Ok(vec![RunnerMessage::RuntimeStatus {
                    status: RuntimeStatus {
                        state: RuntimeStatusState::Paused,
                        transport: RuntimeTransportState::Stopped,
                        current_ppqn_pulse: 0,
                        pending_resync: false,
                        sync_source: SyncSource::Internal,
                        message: None,
                        error: None,
                    },
                }]),
                HostMessage::DeviceInput { .. } | HostMessage::MidiRealtimeContinue => Ok(vec![]),
            }
        }
    }

    #[derive(Default)]
    struct FakeHost {
        midi_messages: Vec<Vec<u8>>,
        musical_events: Vec<MusicalEvent>,
        audio_commands: Vec<RuntimeAudioCommand>,
        effects: Vec<RuntimePlatformEffect>,
        silence_calls: usize,
        fail_internal_silence: bool,
    }

    impl HostAdapter for FakeHost {
        fn handle_musical_event(
            &mut self,
            event: &MusicalEvent,
        ) -> Result<(), RuntimeAdapterError> {
            self.musical_events.push(event.clone());
            Ok(())
        }

        fn handle_platform_effect(
            &mut self,
            request: &RuntimePlatformRequest,
        ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
            let effect = &request.effect;
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

        fn handle_audio_command(
            &mut self,
            command: &RuntimeAudioCommand,
        ) -> Result<(), RuntimeAdapterError> {
            self.audio_commands.push(command.clone());
            Ok(())
        }

        fn handle_midi_message(&mut self, bytes: &[u8]) -> Result<(), RuntimeAdapterError> {
            self.midi_messages.push(bytes.to_vec());
            Ok(())
        }

        fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError> {
            self.silence_calls += 1;
            if self.fail_internal_silence {
                Err(RuntimeAdapterError::from("internal audio silence failed"))
            } else {
                Ok(())
            }
        }

        fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
            self.handle_midi_message(&[0xFC])?;
            for channel in 0..16_u8 {
                self.handle_midi_message(&[0xB0 | channel, 120, 0])?;
                self.handle_midi_message(&[0xB0 | channel, 123, 0])?;
            }
            Ok(())
        }
    }

    fn set_runtime_playing(runtime: &mut PlaybackRuntime, host: &mut FakeHost) {
        runtime
            .ingest_runner_messages(
                vec![RunnerMessage::RuntimeStatus {
                    status: RuntimeStatus {
                        state: RuntimeStatusState::Running,
                        transport: RuntimeTransportState::Playing,
                        current_ppqn_pulse: 0,
                        pending_resync: false,
                        sync_source: SyncSource::Internal,
                        message: None,
                        error: None,
                    },
                }],
                host,
            )
            .unwrap();
        host.midi_messages.clear();
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
        set_runtime_playing(&mut runtime, &mut host);

        runtime.advance(500, &mut runner, &mut host).unwrap();

        assert!(matches!(
            runner.seen[0],
            HostMessage::TransportPulseStep { pulses: 24, .. }
        ));
        assert_eq!(host.midi_messages[0], vec![0x90, 60, 100]);
        assert_eq!(host.midi_messages[1], vec![0xF8]);
        assert_eq!(host.midi_messages.len(), 25);

        runtime.advance(30, &mut runner, &mut host).unwrap();
        assert!(host.midi_messages.contains(&vec![0x80, 60, 0]));
    }

    #[test]
    fn internal_clock_preserves_sub_millisecond_transport_time() {
        let mut runtime = PlaybackRuntime::new(RuntimeConfig {
            bpm: 120.0,
            sync_source: SyncSource::Internal,
            midi_clock_out_enabled: false,
            midi_out_enabled: false,
        });
        let mut runner = FakeRunner::default();
        let mut host = FakeHost::default();
        set_runtime_playing(&mut runtime, &mut host);

        for _ in 0..100 {
            runtime
                .advance_duration(
                    std::time::Duration::from_micros(8_900),
                    &mut runner,
                    &mut host,
                )
                .unwrap();
        }

        let pulses = runner
            .seen
            .iter()
            .filter_map(|message| match message {
                HostMessage::TransportPulseStep { pulses, .. } => Some(*pulses),
                _ => None,
            })
            .sum::<u32>();
        assert_eq!(pulses, 42);
    }

    #[test]
    fn midi_only_events_send_midi_without_host_audio_and_schedule_note_off() {
        struct MidiOnlyRunner;

        impl CoreRunner for MidiOnlyRunner {
            fn send(&mut self, _message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
                Ok(vec![RunnerMessage::MidiEvents {
                    events: vec![MusicalEvent::NoteOn {
                        channel: 1,
                        note: 64,
                        velocity: 90,
                        duration_ms: Some(30),
                    }],
                }])
            }
        }

        let mut runtime = PlaybackRuntime::new(RuntimeConfig {
            bpm: 120.0,
            sync_source: SyncSource::Internal,
            midi_clock_out_enabled: false,
            midi_out_enabled: true,
        });
        let mut runner = MidiOnlyRunner;
        let mut host = FakeHost::default();
        set_runtime_playing(&mut runtime, &mut host);

        runtime.advance(500, &mut runner, &mut host).unwrap();
        runtime.advance(30, &mut runner, &mut host).unwrap();

        assert!(host.musical_events.is_empty());
        assert_eq!(host.midi_messages[0], vec![0x91, 64, 90]);
        assert!(host.midi_messages.contains(&vec![0x81, 64, 0]));
    }

    #[test]
    fn external_midi_realtime_bytes_preserve_clock_control_order() {
        let mut runtime = PlaybackRuntime::new(RuntimeConfig {
            bpm: 120.0,
            sync_source: SyncSource::External,
            midi_clock_out_enabled: true,
            midi_out_enabled: true,
        });
        let mut runner = FakeRunner::default();
        let mut host = FakeHost::default();

        runtime
            .handle_midi_realtime_bytes(
                &[0xF8, 0xF8, 0xFA, 0xF8, 0xFB, 0xF8, 0xFC],
                &mut runner,
                &mut host,
            )
            .unwrap();

        assert_eq!(runner.seen.len(), 6);
        assert!(matches!(
            runner.seen[0],
            HostMessage::MidiRealtimeClock { pulses: 2 }
        ));
        assert!(matches!(runner.seen[1], HostMessage::MidiRealtimeStart));
        assert!(matches!(
            runner.seen[2],
            HostMessage::MidiRealtimeClock { pulses: 1 }
        ));
        assert!(matches!(runner.seen[3], HostMessage::MidiRealtimeContinue));
        assert!(matches!(
            runner.seen[4],
            HostMessage::MidiRealtimeClock { pulses: 1 }
        ));
        assert!(matches!(runner.seen[5], HostMessage::MidiRealtimeStop));
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
        set_runtime_playing(&mut runtime, &mut host);

        runtime.advance(500, &mut runner, &mut host).unwrap();

        assert_eq!(host.effects, vec![RuntimePlatformEffect::StoreListPresets]);
        assert_eq!(
            runtime.last_snapshot(),
            Some(&json!({
                "roundTrip": {
                    "type": "identified",
                    "requestId": "platform-1",
                    "revision": null,
                    "result": {
                        "type": "list_presets_result",
                        "names": ["Factory", "Live Set"]
                    }
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
            request_snapshot: None,
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
        assert_eq!(host.silence_calls, 0);
    }

    #[test]
    fn runtime_errors_decorate_presentations_and_preserve_last_good_state() {
        let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
        let mut host = FakeHost::default();
        let good_snapshot = json!({ "transport": { "tick": 1 } });
        let later_snapshot = json!({ "transport": { "tick": 2 } });
        let good_status = RuntimeStatus {
            state: RuntimeStatusState::Running,
            transport: RuntimeTransportState::Playing,
            current_ppqn_pulse: 24,
            pending_resync: false,
            sync_source: SyncSource::Internal,
            message: None,
            error: None,
        };

        runtime
            .ingest_runner_messages(
                vec![
                    RunnerMessage::Snapshot {
                        snapshot: good_snapshot.clone(),
                    },
                    RunnerMessage::RuntimeStatus {
                        status: good_status.clone(),
                    },
                ],
                &mut host,
            )
            .unwrap();

        let error = RuntimeErrorMetadata {
            domain: RuntimeErrorDomain::Storage,
            code: RuntimeErrorCode::OperationFailed,
            operation: RuntimeOperation::StoreLoadDefault,
            recovery: RuntimeRecovery::RetainLastGood,
            request_id: Some("load-default-1".into()),
            revision: Some(4),
            message: Some("disk full".into()),
        };
        runtime
            .ingest_runner_messages(
                vec![RunnerMessage::RuntimeStatus {
                    status: RuntimeStatus {
                        state: RuntimeStatusState::Error,
                        transport: RuntimeTransportState::Playing,
                        current_ppqn_pulse: 24,
                        pending_resync: false,
                        sync_source: SyncSource::Internal,
                        message: Some("disk full".into()),
                        error: Some(error.clone()),
                    },
                }],
                &mut host,
            )
            .unwrap();

        assert_eq!(runtime.last_good_snapshot(), Some(&good_snapshot));
        assert_eq!(runtime.last_good_status(), Some(&good_status));
        assert_eq!(
            runtime.last_status().unwrap().state,
            RuntimeStatusState::Error
        );
        assert_eq!(runtime.last_status().unwrap().error, Some(error.clone()));
        assert_eq!(
            runtime.last_snapshot(),
            Some(&json!({
                "transport": { "tick": 1 },
                "runtimeError": {
                    "domain": "storage",
                    "code": "operation_failed",
                    "operation": "store_load_default",
                    "recovery": "retain_last_good",
                    "requestId": "load-default-1",
                    "revision": 4,
                    "message": "disk full"
                }
            }))
        );

        runtime
            .ingest_runner_messages(
                vec![
                    RunnerMessage::Snapshot {
                        snapshot: later_snapshot.clone(),
                    },
                    RunnerMessage::RuntimeStatus {
                        status: good_status,
                    },
                ],
                &mut host,
            )
            .unwrap();
        assert_eq!(runtime.last_good_snapshot(), Some(&later_snapshot));
        assert_eq!(
            runtime.last_snapshot(),
            Some(&json!({
                "transport": { "tick": 2 },
                "runtimeError": {
                    "domain": "storage",
                    "code": "operation_failed",
                    "operation": "store_load_default",
                    "recovery": "retain_last_good",
                    "requestId": "load-default-1",
                    "revision": 4,
                    "message": "disk full"
                }
            }))
        );

        runtime.ingest_runtime_result(&RuntimeStoreResult::OperationSucceeded {
            operation: RuntimeOperation::StoreLoadDefault,
            request_id: Some("load-default-1".into()),
            revision: Some(4),
        });
        assert!(runtime.latched_errors().is_empty());
        assert_eq!(
            runtime.last_status().unwrap().state,
            RuntimeStatusState::Running
        );
        assert_eq!(runtime.last_snapshot(), Some(&later_snapshot));

        runtime.ingest_runtime_result(&RuntimeStoreResult::SampleListError {
            instrument_slot: 0,
            sample_slot: 0,
            dir: "samples".into(),
            message: "permission denied".into(),
        });
        assert_eq!(
            runtime.latched_errors()[0].operation,
            RuntimeOperation::SampleList
        );
        runtime.ingest_runtime_result(&RuntimeStoreResult::SampleListResult {
            instrument_slot: 0,
            sample_slot: 0,
            dir: "samples".into(),
            entries: Vec::new(),
        });
        assert!(runtime.latched_errors().is_empty());
    }

    #[test]
    fn stop_and_silence_stops_runner_and_panics_all_routes() {
        let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
        let mut runner = FakeRunner::default();
        let mut host = FakeHost::default();
        set_runtime_playing(&mut runtime, &mut host);
        host.midi_messages.clear();

        runtime
            .recover_from_error(
                RuntimeErrorMetadata::operation_failed(
                    RuntimeErrorDomain::Runtime,
                    RuntimeOperation::RuntimeDispatch,
                    RuntimeRecovery::StopAndSilence,
                    "runner failed".into(),
                ),
                &mut runner,
                &mut host,
            )
            .unwrap();

        assert!(matches!(
            runner.seen.last(),
            Some(HostMessage::TransportStop)
        ));
        assert_eq!(host.midi_messages.len(), 34);
        assert_eq!(host.silence_calls, 1);
        assert_eq!(
            runtime.last_status().unwrap().transport,
            RuntimeTransportState::Stopped
        );
        assert_eq!(
            runtime.last_status().unwrap().state,
            RuntimeStatusState::Error
        );
    }

    #[test]
    fn safety_operation_continues_to_external_midi_when_internal_silence_fails() {
        let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
        let mut runner = FakeRunner::default();
        let mut host = FakeHost {
            fail_internal_silence: true,
            ..FakeHost::default()
        };
        set_runtime_playing(&mut runtime, &mut host);
        host.midi_messages.clear();

        runtime
            .recover_from_error(
                RuntimeErrorMetadata::operation_failed(
                    RuntimeErrorDomain::Runtime,
                    RuntimeOperation::RuntimeDispatch,
                    RuntimeRecovery::StopAndSilence,
                    "runner failed".into(),
                ),
                &mut runner,
                &mut host,
            )
            .unwrap();

        assert!(runner
            .seen
            .iter()
            .any(|message| matches!(message, HostMessage::TransportStop)));
        assert_eq!(host.silence_calls, 1);
        assert_eq!(host.midi_messages.len(), 34);
        assert!(runtime
            .latched_errors()
            .iter()
            .any(|error| error.domain == RuntimeErrorDomain::Audio));
    }

    #[test]
    fn malformed_snapshot_is_typed_safe_and_requests_stop_without_panicking() {
        let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
        let mut host = FakeHost::default();
        runtime
            .ingest_runner_messages(
                vec![RunnerMessage::Snapshot {
                    snapshot: json!({ "trusted": true }),
                }],
                &mut host,
            )
            .unwrap();
        host.midi_messages.clear();

        let output = runtime
            .ingest_runner_messages_with_output(
                vec![RunnerMessage::Snapshot {
                    snapshot: json!(42),
                }],
                &mut host,
            )
            .unwrap();

        assert_eq!(
            runtime.last_good_snapshot(),
            Some(&json!({ "trusted": true }))
        );
        assert_eq!(
            runtime.latched_errors()[0].operation,
            RuntimeOperation::Snapshot
        );
        assert_eq!(
            runtime.latched_errors()[0].domain,
            RuntimeErrorDomain::Serialization
        );
        assert_eq!(host.midi_messages.len(), 33);
        assert!(output
            .follow_ups
            .iter()
            .any(|message| matches!(message, HostMessage::TransportStop)));
    }

    #[test]
    fn fault_clear_requires_matching_request_and_revision_identity() {
        let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
        let make_error = |revision| {
            RuntimeErrorMetadata::operation_failed(
                RuntimeErrorDomain::Audio,
                RuntimeOperation::AudioCommand,
                RuntimeRecovery::RetainLastGood,
                "audio queue failed".into(),
            )
            .with_identity(Some("audio-request".into()), Some(revision))
        };

        runtime.latch_error(make_error(1));
        runtime.latch_error(make_error(2));
        runtime.clear_error_with_identity(
            RuntimeOperation::AudioCommand,
            Some("audio-request"),
            Some(1),
        );

        assert_eq!(runtime.latched_errors().len(), 1);
        assert_eq!(runtime.latched_errors()[0].revision, Some(2));
        runtime.ingest_runtime_result(&RuntimeStoreResult::OperationSucceeded {
            operation: RuntimeOperation::AudioCommand,
            request_id: Some("audio-request".into()),
            revision: Some(2),
        });
        assert!(runtime.latched_errors().is_empty());
    }

    #[test]
    fn identified_audio_prep_failure_retains_last_good_snapshot() {
        let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
        let mut host = FakeHost::default();
        let snapshot = json!({ "audio": { "revision": 3 } });
        runtime
            .ingest_runner_messages(
                vec![RunnerMessage::Snapshot {
                    snapshot: snapshot.clone(),
                }],
                &mut host,
            )
            .unwrap();

        runtime.ingest_runtime_result(&RuntimeStoreResult::Identified {
            result: Box::new(RuntimeStoreResult::RuntimeFailure {
                error: RuntimeErrorFacts::new(
                    RuntimeErrorDomain::Audio,
                    RuntimeErrorCode::InvalidPayload,
                    RuntimeOperation::AudioCommand,
                    Some("sample decode failed".into()),
                ),
            }),
            request_id: "audio-3".into(),
            revision: Some(3),
        });

        assert_eq!(runtime.last_good_snapshot(), Some(&snapshot));
        assert_eq!(
            runtime.latched_errors()[0].operation,
            RuntimeOperation::AudioCommand
        );
        assert_eq!(
            runtime.latched_errors()[0].code,
            RuntimeErrorCode::InvalidPayload
        );
        assert_eq!(
            runtime.latched_errors()[0].recovery,
            RuntimeRecovery::RetainLastGood
        );
    }

    #[test]
    fn worker_emission_and_persistence_faults_do_not_safety_stop() {
        let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
        let mut runner = FakeRunner::default();
        let mut host = FakeHost::default();

        runtime
            .recover_from_facts(
                RuntimeErrorFacts::new(
                    RuntimeErrorDomain::Runtime,
                    RuntimeErrorCode::OperationFailed,
                    RuntimeOperation::RuntimeEmission,
                    Some("emit failed".into()),
                ),
                &mut runner,
                &mut host,
            )
            .unwrap();
        assert!(runner.seen.is_empty());
        assert_eq!(
            runtime.latched_errors()[0].recovery,
            RuntimeRecovery::RetainLastGood
        );

        runtime.clear_error(RuntimeOperation::RuntimeEmission);
        runtime
            .recover_from_facts(
                RuntimeErrorFacts::new(
                    RuntimeErrorDomain::Storage,
                    RuntimeErrorCode::OperationFailed,
                    RuntimeOperation::Persistence,
                    Some("save failed".into()),
                ),
                &mut runner,
                &mut host,
            )
            .unwrap();
        assert!(runner.seen.is_empty());
        assert_eq!(runtime.latched_errors()[0].recovery, RuntimeRecovery::Retry);
    }
}
