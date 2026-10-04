use crate::audio::AudioService;
use crate::hardware_runtime_scheduler::HardwareRuntimeScheduler;
use crate::host_adapter::PiHostAdapter;
use crate::input::MidiMessage;
use crate::keyboard_capture::KeyboardCapture;
use crate::main_runtime_loop::{run_runtime_loop, BoardLoop, LoopInputs, LoopState};
use crate::render_loop::RenderWorker;
use crate::runtime_output::process_runtime_output;
use crate::timing_input::{fail_study, TimingInput};
use octessera_hal::encoder_gpio::HardwareEvent;
use playback_runtime::{AudioOptimization, HostMessage, NativeRunner, PlaybackRuntime};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Instant;

#[path = "runtime_startup.rs"]
mod startup;
#[cfg(test)]
#[path = "raspberry_timing_input_tests.rs"]
mod timing_input_tests;
pub(crate) use startup::{prepare, PreparedRuntime};

pub(crate) struct RaspberryRuntimeConfig {
    pub(crate) audio: Option<AudioService>,
    pub(crate) store_dir: PathBuf,
    pub(crate) samples_dir: PathBuf,
    pub(crate) midi_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
    pub(crate) usb_midi_out_enabled: bool,
    pub(crate) audio_outputs: playback_runtime::AudioOutputSet,
    pub(crate) usb_data_role: playback_runtime::UsbDataRole,
    pub(crate) audio_optimization: AudioOptimization,
    #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
    pub(crate) audio_load_rx: Option<rodio_engine_source::AudioLoadStatusReceiver>,
    pub(crate) midi_rx: mpsc::Receiver<MidiMessage>,
    pub(crate) input_rx: mpsc::Receiver<HostMessage>,
    pub(crate) encoder_rx: mpsc::Receiver<HardwareEvent>,
    pub(crate) early_boot_splash: bool,
    pub(crate) keyboard: KeyboardCapture,
}

pub(crate) fn run(config: RaspberryRuntimeConfig, render_worker: RenderWorker) {
    match prepare(config) {
        Ok(prepared) => prepared.run(render_worker),
        Err(error) => {
            eprintln!("pi runtime preparation failed: {error}");
            let _ = render_worker.publish_shutdown();
        }
    }
}

fn run_scheduler(
    prepared: PreparedRuntime,
    render_worker: RenderWorker,
    initial_rendered_revision: u64,
) {
    crate::runtime_thread::run_on_runtime_thread(move || {
        scheduler_loop(prepared, render_worker, initial_rendered_revision)
    });
}

fn scheduler_loop(
    prepared: PreparedRuntime,
    render_worker: RenderWorker,
    initial_rendered_revision: u64,
) {
    let PreparedRuntime {
        midi_rx,
        input_rx,
        encoder_rx,
        mut playback,
        mut runner,
        mut adapter,
        candidate_readiness: _,
        keyboard,
        #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
        audio_load_rx,
    } = prepared;
    let timing = crate::timing_input::validate_startup(&playback).and_then(|auto_aux| {
        TimingInput::prepare(auto_aux, &mut playback, &mut runner, &mut adapter)
    });
    let timing = match timing {
        Ok(timing) => timing,
        Err(error) if std::env::var_os("OCTESSERA_TIMING_AUTOAUX").is_some() => {
            fail_study::<PiHostAdapter>(error)
        }
        Err(error) => {
            eprintln!("pi timing input setup failed: {error}");
            let _ = keyboard.shutdown();
            let _ = render_worker.publish_shutdown();
            return;
        }
    };
    let scheduler = HardwareRuntimeScheduler::new(Instant::now(), initial_rendered_revision);
    let mut state = LoopState::new(scheduler, timing);
    let mut board = RaspberryBoardLoop {
        audio: adapter.audio_service(),
        #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
        audio_load_rx,
    };
    let inputs = LoopInputs {
        midi_rx: &midi_rx,
        input_rx: &input_rx,
        encoder_rx: &encoder_rx,
    };
    let result = run_runtime_loop(
        &mut state,
        &mut playback,
        &mut runner,
        &mut adapter,
        &render_worker,
        &inputs,
        &mut board,
    );
    if let Err(message) = result {
        present_audio_fault(
            message,
            &mut playback,
            &mut runner,
            &mut adapter,
            &render_worker,
        );
    }
    if let Err(error) = keyboard.shutdown() {
        eprintln!("USB keyboard worker shutdown failed: {error}");
    }
}

struct RaspberryBoardLoop {
    audio: Option<AudioService>,
    #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
    audio_load_rx: Option<rodio_engine_source::AudioLoadStatusReceiver>,
}

impl BoardLoop for RaspberryBoardLoop {
    fn service_audio(
        &mut self,
        _playback: &mut PlaybackRuntime,
        _runner: &mut NativeRunner,
        _adapter: &mut PiHostAdapter,
    ) -> Result<(), String> {
        #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
        if let Some(load_rx) = self.audio_load_rx.as_ref() {
            let output = crate::audio::drain_audio_load_status(load_rx, _playback, false);
            if let Err(error) = process_runtime_output(_playback, _runner, _adapter, output) {
                crate::runtime_loop::report_runtime_failure(
                    _adapter,
                    "pi audio load-status output processing failed",
                    error,
                );
            }
        }
        match &self.audio {
            Some(audio) if audio.required_jack_failed() => {
                Err("required Jack audio stream faulted".into())
            }
            _ => Ok(()),
        }
    }

    fn handle_power_request(
        &mut self,
        playback: &PlaybackRuntime,
        adapter: &mut PiHostAdapter,
        render_worker: &RenderWorker,
    ) -> bool {
        crate::raspberry_power::shutdown_if_requested(playback, adapter, render_worker)
    }
}

fn present_audio_fault(
    message: String,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiHostAdapter,
    render_worker: &RenderWorker,
) {
    let error = playback_runtime::RuntimeErrorFacts::new(
        playback_runtime::RuntimeErrorDomain::Audio,
        playback_runtime::RuntimeErrorCode::AudioThreadFailed,
        playback_runtime::RuntimeOperation::AudioThread,
        Some(message),
    );
    match playback.recover_from_facts(error, runner, adapter) {
        Ok(output) => {
            if let Err(error) = process_runtime_output(playback, runner, adapter, output) {
                eprintln!("pi audio fault output processing failed: {error}");
            }
        }
        Err(error) => eprintln!("pi audio fault recovery failed: {error}"),
    }
    let snapshot = playback.last_snapshot().cloned();
    if let Some(snapshot) = snapshot {
        if let Ok(oled) = adapter.core.oled_publication_for_snapshot(&snapshot, false) {
            if let Err(error) = render_worker.publish_snapshot_with_ack(snapshot, oled) {
                eprintln!("pi audio fault snapshot publication failed: {error}");
            }
        }
    }
    let _ = render_worker.publish_shutdown();
}
