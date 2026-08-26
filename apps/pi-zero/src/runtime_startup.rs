use super::RuntimeThreadConfig;
use crate::candidate_readiness::CandidateReadiness;
use crate::host_adapter::PiPlaybackHostAdapter;
use crate::initial_audio_prep::{interpret_initial_audio_prep, InitialAudioPrepBoard};
use crate::input::MidiMessage;
use crate::main_paths::ensure_samples_dir;
use crate::normal_menu::is_normal_menu_snapshot;
use crate::render_loop::RenderWorker;
use crate::runtime_loop::initialize_host_state;
use crate::sample_browser::builtin_favourite_dirs;
use octessera_hal::encoder_gpio::HardwareEvent;
use playback_runtime::{
    HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RuntimeConfig, SyncSource,
};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const INITIAL_AUDIO_PREP_TIMEOUT: Duration = Duration::from_secs(10);
const INITIAL_AUDIO_PREP_POLL: Duration = Duration::from_millis(10);

pub(crate) struct PreparedRuntime {
    pub(super) midi_rx: mpsc::Receiver<MidiMessage>,
    pub(super) input_rx: mpsc::Receiver<HostMessage>,
    pub(super) encoder_rx: mpsc::Receiver<HardwareEvent>,
    pub(super) playback: PlaybackRuntime,
    pub(super) runner: NativeRunner,
    pub(super) adapter: PiPlaybackHostAdapter,
    pub(super) candidate_readiness: CandidateReadiness,
}

pub(crate) fn prepare(config: RuntimeThreadConfig) -> Result<PreparedRuntime, String> {
    let RuntimeThreadConfig {
        audio,
        store_dir,
        samples_dir,
        midi_handler,
        usb_midi_out_enabled,
        audio_outputs,
        midi_rx,
        input_rx,
        encoder_rx,
        early_boot_splash,
    } = config;
    ensure_samples_dir(&samples_dir)?;
    let (mut playback, mut runner) = init_runtime();
    if early_boot_splash {
        runner.skip_startup_splash();
    }
    let mut adapter = PiPlaybackHostAdapter::new(
        audio,
        store_dir,
        samples_dir,
        midi_handler,
        usb_midi_out_enabled,
        audio_outputs,
    );
    initialize_host_state(&mut playback, &mut runner, &mut adapter)?;
    let message = HostMessage::TransportPulseStep {
        pulses: 0,
        source: playback.config().sync_source.clone(),
        at_ppqn_pulse: playback
            .last_status()
            .map(|status| status.current_ppqn_pulse),
        request_snapshot: Some(true),
    };
    crate::runtime_loop::dispatch_runtime_message(
        &mut playback,
        &mut runner,
        &mut adapter,
        message,
    )?;
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
    })
}

fn wait_for_initial_audio_prep(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
) -> Result<(), String> {
    let deadline = Instant::now() + INITIAL_AUDIO_PREP_TIMEOUT;
    loop {
        let audio = adapter
            .audio_service()
            .expect("initial Pi audio preparation requires an audio service");
        if let Some(message) = audio.drain_prep_results(1).into_iter().next() {
            let outcome = interpret_initial_audio_prep(
                &message,
                audio
                    .config_revision
                    .load(std::sync::atomic::Ordering::SeqCst),
                InitialAudioPrepBoard::Pi,
            );
            crate::runtime_loop::dispatch_runtime_message(playback, runner, adapter, message)?;
            if let Some(outcome) = outcome {
                return outcome;
            }
        }
        for message in adapter.drain_platform_results(4) {
            let outcome = interpret_initial_audio_prep(
                &message,
                adapter
                    .audio_service()
                    .expect("initial Pi audio preparation requires an audio service")
                    .config_revision
                    .load(std::sync::atomic::Ordering::SeqCst),
                InitialAudioPrepBoard::Pi,
            );
            crate::runtime_loop::dispatch_runtime_message(playback, runner, adapter, message)?;
            if let Some(outcome) = outcome {
                return outcome;
            }
        }
        if Instant::now() >= deadline {
            return Err("initial Pi audio preparation timed out".into());
        }
        std::thread::sleep(INITIAL_AUDIO_PREP_POLL);
    }
}

impl PreparedRuntime {
    pub(crate) fn publish_acknowledged_snapshot(
        &mut self,
        render_worker: &RenderWorker,
    ) -> Result<u64, String> {
        let snapshot = self
            .playback
            .last_snapshot()
            .cloned()
            .ok_or_else(|| "pi initial snapshot is missing".to_string())?;
        if !is_normal_menu_snapshot(&snapshot) {
            return Err("pi initial snapshot is not a normal menu".into());
        }
        if !self.runner.is_canonical_menu_presentation() {
            return Err("pi native runner is not presenting the canonical menu".into());
        }
        let revision = self.playback.last_snapshot_revision();
        if revision == 0 {
            return Err("pi initial snapshot revision is missing".into());
        }
        let oled = self
            .adapter
            .oled_publication_for_snapshot(&snapshot, true)?;
        render_worker.publish_acknowledged_snapshot(snapshot, oled)?;
        Ok(revision)
    }

    pub(crate) fn mark_candidate_ready(&mut self) -> Result<(), String> {
        if let Some(audio) = self.adapter.audio_service() {
            audio.ensure_route_readiness()?;
        }
        self.candidate_readiness.mark_ready()
    }

    pub(crate) fn spawn_after_initial(
        self,
        render_worker: RenderWorker,
        revision: u64,
    ) -> JoinHandle<()> {
        std::thread::Builder::new()
            .name("octessera-runtime".into())
            .spawn(move || super::run_scheduler(self, render_worker, revision))
            .expect("pi runtime thread should start")
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

fn init_runtime() -> (PlaybackRuntime, NativeRunner) {
    let playback = PlaybackRuntime::new(RuntimeConfig {
        bpm: 120.0,
        sync_source: SyncSource::Internal,
        midi_clock_out_enabled: false,
        midi_out_enabled: false,
    });
    let runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        sample_builtin_favourite_dirs: builtin_favourite_dirs(),
        ..NativeRunnerConfig::default()
    })
    .expect("native runner should initialize");
    (playback, runner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::test_service_with_prep_result_sender;
    use crate::candidate_readiness::CandidateReadiness;
    use playback_runtime::{RuntimeOperation, RuntimeStoreResult};
    use std::sync::Arc;

    #[test]
    fn pi_startup_waits_for_the_identified_audio_prep_result() {
        let (audio, result_tx) = test_service_with_prep_result_sender();
        let root = std::env::temp_dir().join(format!(
            "octessera-pi-startup-prep-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut adapter = PiPlaybackHostAdapter::new(
            Some(audio),
            root.join("store"),
            root.join("samples"),
            Arc::new(|_| {}),
            false,
            playback_runtime::AudioOutputSet::jack(),
        );
        let (mut playback, mut runner) = init_runtime();
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
        let root = std::env::temp_dir().join(format!(
            "octessera-pi-startup-prep-failure-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut adapter = PiPlaybackHostAdapter::new(
            Some(audio),
            root.join("store"),
            root.join("samples"),
            Arc::new(|_| {}),
            false,
            playback_runtime::AudioOutputSet::jack(),
        );
        let (mut playback, mut runner) = init_runtime();
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
        let root = std::env::temp_dir().join(format!(
            "octessera-pi-route-readiness-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let adapter = PiPlaybackHostAdapter::new(
            Some(audio),
            root.join("store"),
            root.join("samples"),
            Arc::new(|_| {}),
            false,
            playback_runtime::AudioOutputSet::jack(),
        );
        let (playback, runner) = init_runtime();
        let (_, midi_rx) = mpsc::channel::<MidiMessage>();
        let (_, input_rx) = mpsc::channel::<HostMessage>();
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
        };

        let error = prepared.mark_candidate_ready().unwrap_err();

        assert_eq!(error, "selected Jack audio route is not active");
        assert!(!marker.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn pi_startup_uses_canonical_builtin_sample_favourites() {
        let (_, mut runner) = init_runtime();

        crate::sample_browser::assert_builtin_favourite_menu(&mut runner);
    }
}

#[cfg(test)]
#[path = "runtime_startup_prep_tests.rs"]
mod prep_tests;
