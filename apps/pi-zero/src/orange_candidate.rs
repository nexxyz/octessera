use crate::audio::{AudioManager, AudioService};
use crate::audio_stream_health::AudioStreamStatus as OrangeDacStatus;
use crate::boot_oled_handoff::{HandoffMode, StartupFatalCode};
use crate::candidate_readiness::CandidateReadiness;
use crate::encoder_queue::PendingEncoderTurns;
use crate::hardware_runtime_scheduler::{
    prepare_dispatch_message, DisplaySnapshotDue, HardwareRuntimeScheduler,
};
use crate::input::{midi_realtime_message, MidiMessage};
use crate::main_paths::default_store_dir;
use crate::midi_host::drain_midi_messages;
use crate::normal_menu::is_normal_menu_snapshot;
pub(crate) use crate::orange_device_apply::OrangeRunError;
use crate::orange_host_adapter::OrangeHostAdapter;
use crate::power_lifecycle::{PowerAction, PowerLifecycleResult};
use crate::render::HardwareRenderTargets;
use crate::render_loop::RenderWorker;
use crate::seesaw_io::{self, SeesawIo};
use crate::usb_config::read_usb_runtime_config;
use octessera_hal::board_profiles::{SeesawInputMode, ORANGE_PI_ZERO_2W_DEVICES};
use octessera_hal::encoder_gpio::HardwareEvent;
use octessera_hal::{NeoKey, NeoTrellis, OledSsd1351, OrangeEncoderGpio};
use playback_runtime::{
    HostAdapter, HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RuntimeConfig,
    SyncSource,
};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[path = "orange_candidate_handoff.rs"]
mod handoff;
#[path = "orange_lifecycle.rs"]
mod lifecycle;
#[path = "orange_runtime_loop.rs"]
mod runtime_loop;
#[path = "orange_signal.rs"]
mod signal;
#[path = "orange_runtime_startup.rs"]
mod startup;
#[cfg(test)]
pub(crate) use runtime_loop::drain_host_results;
pub(crate) use runtime_loop::{dispatch, process_runtime_output, run_prepared_runtime};
pub(crate) use startup::{
    prepare_runtime, publish_prepared_acknowledged_snapshot, wait_for_initial_audio_prep,
    OrangeStartupReadinessGate, PreparedRuntime,
};
const HOST_RESULT_BUDGET: usize = 4;
const ORANGE_UART0_ACTIVE: bool = false;

fn startup_failure(
    mode: HandoffMode,
    code: StartupFatalCode,
    error: impl Into<String>,
) -> OrangeRunError {
    let error = error.into();
    if mode == HandoffMode::V1 {
        if let Err(signal_error) = crate::boot_oled_handoff::publish_startup_fatal(code) {
            eprintln!("Orange startup fatal signal publish failed: {signal_error}");
        }
    }
    OrangeRunError::Ordinary(error)
}

struct OrangeRuntimeServices {
    midi_rx: Receiver<MidiMessage>,
    midi_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
    usb_midi_out_enabled: bool,
}
pub fn run() -> Result<(), OrangeRunError> {
    let mut candidate_readiness = CandidateReadiness::from_env();
    crate::board_profile::validate_runtime_profile()?;
    let handoff_mode = crate::boot_oled_handoff::mode_from_env()?;
    signal::install_signal_handlers().map_err(|error| {
        startup_failure(
            handoff_mode,
            StartupFatalCode::StartupFailed,
            format!("Orange signal handler startup failed: {error}"),
        )
    })?;
    let store_dir = default_store_dir();
    crate::orange_device_apply::recover_startup(&store_dir).map_err(|error| {
        startup_failure(
            handoff_mode,
            StartupFatalCode::StartupFailed,
            format!("Orange startup recovery failed: {error}"),
        )
    })?;
    let usb_config = read_usb_runtime_config(&store_dir).map_err(|error| {
        startup_failure(
            handoff_mode,
            StartupFatalCode::StartupFailed,
            format!("Orange USB runtime configuration is unavailable: {error}"),
        )
    })?;
    let (midi_tx, midi_rx) = mpsc::channel::<MidiMessage>();
    let midi_handler = Arc::new(move |bytes: Vec<u8>| {
        if let Some(message) = midi_realtime_message(&bytes) {
            let _ = midi_tx.send(message);
        }
    });
    let mut audio = AudioManager::new_orange(
        crate::usb_config::audio_output_buffer_frames_from_default_config(&store_dir),
        usb_config.audio_outputs,
    )
    .map_err(|error| {
        startup_failure(
            handoff_mode,
            handoff::startup_fatal_code(handoff::OrangeStartupOperation::Audio),
            format!("Orange audio startup failed: {error}"),
        )
    })?;
    let hdmi = crate::render::hdmi::HdmiFramebuffer::new();
    if handoff_mode == crate::boot_oled_handoff::HandoffMode::V1 {
        return handoff::run(
            audio,
            usb_config,
            midi_rx,
            midi_handler,
            candidate_readiness,
            hdmi,
        );
    }
    let devices = ORANGE_PI_ZERO_2W_DEVICES;
    let oled = OledSsd1351::new()?;
    let trellis = NeoTrellis::new_with_mode(
        devices.i2c.path,
        devices.trellis_addrs,
        SeesawInputMode::Polling,
    )?;
    let neokey = NeoKey::new_with_mode(
        devices.i2c.path,
        devices.neokey_addr,
        SeesawInputMode::Polling,
    )?;
    let (encoder_rx, _encoders) = init_encoders()?;
    let seesaw = seesaw_io::spawn_polling(trellis, neokey)?;
    let render = RenderWorker::spawn(HardwareRenderTargets {
        oled,
        seesaw_tx: seesaw.command_tx.clone(),
        oled_handoff: None,
        hdmi,
    });
    let audio_service = audio.service();
    let result = run_runtime(
        &seesaw,
        &encoder_rx,
        &render,
        &audio_service,
        &mut audio,
        &mut candidate_readiness,
        OrangeRuntimeServices {
            midi_rx,
            midi_handler,
            usb_midi_out_enabled: usb_config.midi_out_enabled,
        },
    );
    let render_result = lifecycle::teardown_render(&result, &render);
    let seesaw_result = seesaw.shutdown();
    let result = result
        .and_then(|resolution| {
            render_result
                .map(|_| resolution)
                .map_err(OrangeRunError::from)
        })
        .and_then(|resolution| {
            seesaw_result
                .map(|_| resolution)
                .map_err(OrangeRunError::from)
        });
    drop(audio);
    result.and_then(crate::orange_device_apply::finish_shutdown_resolution)
}
#[allow(clippy::too_many_arguments)]
fn run_runtime(
    seesaw: &SeesawIo,
    encoder_rx: &Receiver<HardwareEvent>,
    render: &RenderWorker,
    audio: &AudioService,
    audio_manager: &mut AudioManager,
    candidate_readiness: &mut CandidateReadiness,
    services: OrangeRuntimeServices,
) -> Result<crate::orange_device_apply::OrangeShutdownResolution, OrangeRunError> {
    let OrangeRuntimeServices {
        midi_rx,
        midi_handler,
        usb_midi_out_enabled,
    } = services;
    let prepared = prepare_runtime(audio.clone(), midi_handler, usb_midi_out_enabled, true)?;
    run_prepared_runtime(
        prepared,
        seesaw,
        encoder_rx,
        render,
        audio_manager,
        candidate_readiness,
        midi_rx,
        false,
    )
}

fn ensure_required_audio_health(status: OrangeDacStatus) -> Result<(), String> {
    if status == OrangeDacStatus::Terminal {
        Err("Orange Jack audio stream faulted".into())
    } else {
        Ok(())
    }
}
fn init_encoders() -> Result<(Receiver<HardwareEvent>, Vec<OrangeEncoderGpio>), String> {
    let (event_tx, event_rx) = mpsc::channel();
    let mut encoders = Vec::new();
    for (index, pins) in ORANGE_PI_ZERO_2W_DEVICES.encoders.iter().enumerate() {
        let id = encoder_id(index)?;
        encoders.push(OrangeEncoderGpio::new_with_uart0_active(
            id,
            pins,
            ORANGE_UART0_ACTIVE,
            event_tx.clone(),
        )?);
    }
    Ok((event_rx, encoders))
}
#[cfg(test)]
fn qualified_encoder_ids() -> Vec<&'static str> {
    ORANGE_PI_ZERO_2W_DEVICES
        .encoders
        .iter()
        .enumerate()
        .map(|(index, _)| encoder_id(index).expect("Orange encoder descriptor index is valid"))
        .collect()
}
fn encoder_id(index: usize) -> Result<&'static str, String> {
    match index {
        0 => Ok("encoder_main"),
        1 => Ok("encoder_aux_1"),
        2 => Ok("encoder_aux_2"),
        3 => Ok("encoder_aux_3"),
        _ => Err(format!("unknown Orange encoder index {index}")),
    }
}
#[cfg(test)]
#[path = "orange_candidate_tests.rs"]
mod tests;
