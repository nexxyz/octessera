use super::RuntimeThreadConfig;
use crate::candidate_readiness::CandidateReadiness;
use crate::host_adapter::PiPlaybackHostAdapter;
use crate::input::MidiMessage;
use crate::normal_menu::is_normal_menu_snapshot;
use crate::render_loop::RenderWorker;
use crate::runtime_loop::initialize_host_state;
use crate::sample_browser::SD_CARD_SAMPLE_BROWSER_DIR;
use octessera_hal::encoder_gpio::HardwareEvent;
use playback_runtime::{
    HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RuntimeConfig, SyncSource,
};
use std::sync::mpsc;
use std::thread::JoinHandle;

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
        usb_audio_out,
        midi_rx,
        input_rx,
        encoder_rx,
        early_boot_splash,
    } = config;
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
        usb_audio_out,
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
        render_worker.publish_acknowledged_snapshot(snapshot, self.playback.drain_ui_pulses())?;
        Ok(revision)
    }

    pub(crate) fn mark_candidate_ready(&mut self) -> Result<(), String> {
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
        sample_builtin_favourite_dirs: vec![String::new(), SD_CARD_SAMPLE_BROWSER_DIR.into()],
        ..NativeRunnerConfig::default()
    })
    .expect("native runner should initialize");
    (playback, runner)
}
