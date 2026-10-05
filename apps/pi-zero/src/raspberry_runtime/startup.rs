use super::RaspberryRuntimeConfig;
use crate::candidate_readiness::CandidateReadiness;
use crate::host_adapter::PiHostAdapter;
use crate::input::MidiMessage;
use crate::main_paths::ensure_samples_dir;
#[cfg(test)]
use crate::normal_menu::is_normal_menu_snapshot;
use crate::render_loop::RenderWorker;
#[cfg(test)]
use crate::runtime_output::initialize_host_state;
use crate::runtime_output::wait_for_initial_audio_prep;
use octessera_hal::encoder_gpio::HardwareEvent;
#[cfg(test)]
use playback_runtime::{AudioOptimization, NativeRunnerConfig, RuntimeConfig, SyncSource};
use playback_runtime::{HostMessage, NativeRunner, PlaybackRuntime};
use std::sync::mpsc;

pub(crate) struct PreparedRuntime {
    pub(super) midi_rx: mpsc::Receiver<MidiMessage>,
    pub(super) input_rx: mpsc::Receiver<HostMessage>,
    pub(super) encoder_rx: mpsc::Receiver<HardwareEvent>,
    pub(super) playback: PlaybackRuntime,
    pub(super) runner: NativeRunner,
    pub(super) adapter: PiHostAdapter,
    pub(super) candidate_readiness: CandidateReadiness,
    pub(super) keyboard: crate::keyboard_capture::KeyboardCapture,
    #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
    pub(super) audio_load_rx: Option<rodio_engine_source::AudioLoadStatusReceiver>,
}

pub(crate) fn prepare(config: RaspberryRuntimeConfig) -> Result<PreparedRuntime, String> {
    let RaspberryRuntimeConfig {
        audio,
        store_dir,
        samples_dir,
        midi_handler,
        usb_midi_out_enabled,
        audio_outputs,
        usb_data_role,
        audio_optimization,
        #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
        audio_load_rx,
        midi_rx,
        input_rx,
        encoder_rx,
        early_boot_splash,
        keyboard,
    } = config;
    ensure_samples_dir(&samples_dir)?;
    let (mut playback, mut runner) =
        crate::runtime_init::new_runtime(audio_optimization, usb_midi_out_enabled, true)?;
    if early_boot_splash {
        runner.skip_startup_splash();
    }
    let mut adapter = PiHostAdapter::new_with_data_role(
        audio,
        store_dir,
        samples_dir,
        midi_handler,
        usb_midi_out_enabled,
        audio_outputs,
        usb_data_role,
    );
    adapter
        .core
        .set_keyboard_capture_control(keyboard.control());
    crate::runtime_init::start_host(&mut playback, &mut runner, &mut adapter)?;
    if adapter.audio_service().is_some() {
        wait_for_initial_audio_prep(&mut playback, &mut runner, &mut adapter)?;
    }
    Ok(PreparedRuntime {
        midi_rx,
        input_rx,
        encoder_rx,
        playback,
        runner,
        adapter,
        candidate_readiness: CandidateReadiness::from_env(),
        keyboard,
        #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
        audio_load_rx,
    })
}

impl PreparedRuntime {
    pub(crate) fn publish_acknowledged_snapshot(
        &mut self,
        render_worker: &RenderWorker,
    ) -> Result<u64, String> {
        crate::runtime_init::publish_initial_snapshot(
            &self.playback,
            &self.runner,
            &mut self.adapter,
            render_worker,
        )
    }

    pub(crate) fn mark_candidate_ready(&mut self) -> Result<(), String> {
        if let Some(audio) = self.adapter.audio_service() {
            audio.ensure_route_readiness()?;
        }
        self.candidate_readiness.mark_ready()
    }

    pub(crate) fn run_after_initial(self, render_worker: RenderWorker, revision: u64) {
        super::run_scheduler(self, render_worker, revision);
    }

    pub(crate) fn run(mut self, render_worker: RenderWorker) {
        match self.publish_acknowledged_snapshot(&render_worker) {
            Ok(revision) => {
                if let Err(error) = self.mark_candidate_ready() {
                    eprintln!("pi candidate readiness publication failed: {error}");
                    let _ = render_worker.abort();
                    return;
                }
                super::run_scheduler(self, render_worker, revision);
            }
            Err(error) => {
                let _ = render_worker.mark_oled_failed();
                eprintln!("pi initial OLED render failed: {error}");
                let _ = render_worker.abort();
            }
        }
    }
}

#[cfg(test)]
fn init_runtime(
    audio_optimization: AudioOptimization,
    boot_applied_usb_midi_out_enabled: bool,
) -> (PlaybackRuntime, NativeRunner) {
    crate::runtime_init::new_runtime(audio_optimization, boot_applied_usb_midi_out_enabled, true)
        .expect("native runner should initialize")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::test_service_with_prep_result_sender;
    use crate::candidate_readiness::CandidateReadiness;
    use playback_runtime::{RuntimeOperation, RuntimeStoreResult};
    use serde_json::json;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn pi_startup_waits_for_the_identified_audio_prep_result() {
        let (audio, result_tx) = test_service_with_prep_result_sender();
        let root = crate::test_temp_dir::unique_temp_path("octessera-pi-startup-prep");
        let mut adapter = PiHostAdapter::new(
            Some(audio),
            root.join("store"),
            root.join("samples"),
            Arc::new(|_| {}),
            false,
            playback_runtime::AudioOutputSet::jack(),
        );
        let (mut playback, mut runner) = init_runtime(AudioOptimization::Latency, false);
        let marker = root.join("candidate-ready.json");
        let mut readiness = CandidateReadiness::new(Some(marker.clone()), "pi-prep".into());
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            result_tx
                .send(HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::Identified {
                        result: Box::new(RuntimeStoreResult::OperationSucceeded {
                            operation: RuntimeOperation::AudioCommand,
                            request_id: None,
                            revision: Some(0),
                        }),
                        request_id: "audio-initial".into(),
                        revision: Some(0),
                    },
                })
                .unwrap();
        });

        wait_for_initial_audio_prep(&mut playback, &mut runner, &mut adapter).unwrap();

        readiness.mark_ready().unwrap();
        assert!(marker.exists());
        drop(readiness);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn pi_initial_audio_prep_failure_does_not_publish_candidate_ready() {
        let (audio, result_tx) = test_service_with_prep_result_sender();
        let root = crate::test_temp_dir::unique_temp_path("octessera-pi-startup-prep-failure");
        let mut adapter = PiHostAdapter::new(
            Some(audio),
            root.join("store"),
            root.join("samples"),
            Arc::new(|_| {}),
            false,
            playback_runtime::AudioOutputSet::jack(),
        );
        let (mut playback, mut runner) = init_runtime(AudioOptimization::Latency, false);
        let marker = root.join("candidate-ready.json");
        let readiness = CandidateReadiness::new(Some(marker.clone()), "pi-prep-failure".into());
        result_tx
            .send(HostMessage::RuntimeResult {
                result: RuntimeStoreResult::Identified {
                    result: Box::new(RuntimeStoreResult::RuntimeFailure {
                        error: playback_runtime::RuntimeErrorFacts::new(
                            playback_runtime::RuntimeErrorDomain::Sample,
                            playback_runtime::RuntimeErrorCode::NotFound,
                            RuntimeOperation::AudioCommand,
                            Some("sample not found: samples/kick.wav".into()),
                        ),
                    }),
                    request_id: "audio-initial".into(),
                    revision: Some(0),
                },
            })
            .unwrap();

        let error =
            wait_for_initial_audio_prep(&mut playback, &mut runner, &mut adapter).unwrap_err();

        assert!(error.contains("sample not found: samples/kick.wav"));
        assert!(!marker.exists());
        drop(readiness);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn pi_candidate_readiness_rechecks_selected_audio_routes() {
        let (audio, _result_tx) = test_service_with_prep_result_sender();
        let root = crate::test_temp_dir::unique_temp_path("octessera-pi-route-readiness");
        let adapter = PiHostAdapter::new(
            Some(audio),
            root.join("store"),
            root.join("samples"),
            Arc::new(|_| {}),
            false,
            playback_runtime::AudioOutputSet::jack(),
        );
        let (playback, runner) = init_runtime(AudioOptimization::Latency, false);
        let (_, midi_rx) = mpsc::channel::<MidiMessage>();
        let (input_tx, input_rx) = mpsc::channel::<HostMessage>();
        let (_, encoder_rx) = mpsc::channel::<HardwareEvent>();
        let marker = root.join("candidate-ready.json");
        let mut prepared = PreparedRuntime {
            midi_rx,
            input_rx,
            encoder_rx,
            playback,
            runner,
            adapter,
            candidate_readiness: CandidateReadiness::new(
                Some(marker.clone()),
                "pi-route-readiness".into(),
            ),
            keyboard: crate::keyboard_capture::KeyboardCapture::spawn(input_tx, false),
            #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
            audio_load_rx: None,
        };

        let error = prepared.mark_candidate_ready().unwrap_err();

        assert_eq!(error, "selected Jack audio route is not active");
        assert!(!marker.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn pi_startup_uses_canonical_builtin_sample_favourites() {
        let (_, mut runner) = init_runtime(AudioOptimization::Latency, false);

        crate::sample_browser::assert_builtin_favourite_menu(&mut runner);
    }

    #[test]
    fn pi_native_runner_uses_persisted_audio_mode_and_exposes_capacity() {
        let (_, runner) = init_runtime(AudioOptimization::Capacity, false);
        let payload = runner.test_config_payload();

        assert_eq!(payload["runtimeConfig"]["sound"]["optimizeFor"], "capacity");
    }

    #[test]
    fn pi_native_runner_preserves_latency_as_the_default_mode() {
        let (_, runner) = init_runtime(AudioOptimization::Latency, false);
        let payload = runner.test_config_payload();

        assert_eq!(payload["runtimeConfig"]["sound"]["optimizeFor"], "latency");
    }

    #[test]
    fn pi_gadget_midi_startup_waits_for_both_directions_before_oled_handoff() {
        let root = crate::test_temp_dir::unique_temp_path("octessera-pi-gadget-midi-startup");
        let store = root.join("store");
        std::fs::create_dir_all(&store).unwrap();
        let mut payload: serde_json::Value =
            serde_json::from_str(include_str!("../../../../config/generated/pi/default.json"))
                .unwrap();
        payload["runtimeConfig"]["audioOutputs"]["usb"] = json!(true);
        payload["runtimeConfig"]["midi"]["enabled"] = json!(true);
        payload["runtimeConfig"]["usb"]["midiOutEnabled"] = json!(true);
        let documents = playback_runtime::split_system_patch_documents(&payload).unwrap();
        std::fs::write(
            store.join("system.json"),
            serde_json::to_vec(&documents.system).unwrap(),
        )
        .unwrap();
        std::fs::write(
            store.join("default.patch.json"),
            serde_json::to_vec(&documents.patch).unwrap(),
        )
        .unwrap();

        let samples = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples")
            .canonicalize()
            .unwrap();
        let audio = crate::audio::test_service_with_prep_worker();
        let mut adapter = PiHostAdapter::new_with_data_role(
            Some(audio),
            store,
            samples,
            Arc::new(|_| {}),
            true,
            playback_runtime::AudioOutputSet::from_flags(true, true, false).unwrap(),
            playback_runtime::UsbDataRole::Gadget,
        );
        adapter.set_test_midi_backend(
            vec![
                "MIDI Through Port-0".into(),
                "Octessera MIDI:Octessera MIDI 20:0".into(),
            ],
            vec!["MIDI Through Port-0".into(), "Octessera MIDI".into()],
            [Ok(()), Ok(())],
        );
        let (mut playback, mut runner) = init_runtime(AudioOptimization::Latency, true);
        runner.skip_startup_splash();

        initialize_host_state(&mut playback, &mut runner, &mut adapter).unwrap();
        crate::runtime_dispatch::dispatch_runtime_message(
            &mut playback,
            &mut runner,
            &mut adapter,
            HostMessage::TransportPulseStep {
                pulses: 0,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(true),
            },
        )
        .unwrap();
        wait_for_initial_audio_prep(&mut playback, &mut runner, &mut adapter).unwrap();

        let snapshot = playback.last_snapshot().unwrap();
        assert!(playback.latched_errors().is_empty());
        assert!(snapshot["runtimeError"].is_null());
        assert!(is_normal_menu_snapshot(snapshot));
        assert!(runner.is_canonical_menu_presentation());
        assert!(adapter
            .core
            .oled_publication_for_snapshot(snapshot, true)
            .is_ok());
        let _ = std::fs::remove_dir_all(root);
    }
}

#[cfg(test)]
mod prep_tests;
