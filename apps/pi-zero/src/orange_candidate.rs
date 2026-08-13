use crate::audio::{AudioManager, AudioService, OrangeDacStatus};
use crate::candidate_readiness::CandidateReadiness;
use crate::input::{
    encoder_press_message, encoder_turn_message, midi_realtime_message, MidiMessage,
};
use crate::main_paths::default_store_dir;
use crate::midi_host::drain_midi_messages;
use crate::normal_menu::is_normal_menu_snapshot;
use crate::orange_host_adapter::OrangeHostAdapter;
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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[path = "orange_candidate_handoff.rs"]
mod handoff;
#[path = "orange_runtime_startup.rs"]
mod startup;
pub(crate) use startup::{
    prepare_runtime, publish_prepared_acknowledged_snapshot, OrangeStartupReadinessGate,
    PreparedRuntime,
};

const POLLING_INTERVAL: Duration = Duration::from_millis(10);
const RENDER_INTERVAL: Duration = Duration::from_millis(33);
const RUNTIME_TICK: Duration = Duration::from_millis(8);
const HOST_RESULT_BUDGET: usize = 4;
const ORANGE_UART0_ACTIVE: bool = false;

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

struct OrangeRuntimeServices {
    midi_rx: Receiver<MidiMessage>,
    midi_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
    usb_midi_out_enabled: bool,
}

pub fn run() -> Result<(), String> {
    let mut candidate_readiness = CandidateReadiness::from_env();
    crate::board_profile::validate_runtime_profile()?;
    let handoff_mode = crate::boot_oled_handoff::mode_from_env()?;
    install_signal_handlers()?;
    let store_dir = default_store_dir();
    let usb_config = read_usb_runtime_config(&store_dir)
        .map_err(|error| format!("Orange USB runtime configuration is unavailable: {error}"))?;
    let (midi_tx, midi_rx) = mpsc::channel::<MidiMessage>();
    let midi_handler = Arc::new(move |bytes: Vec<u8>| {
        if let Some(message) = midi_realtime_message(&bytes) {
            let _ = midi_tx.send(message);
        }
    });
    let mut audio = AudioManager::new_orange(
        crate::usb_config::audio_output_buffer_frames_from_default_config(&store_dir),
        usb_config.audio_out,
    )
    .map_err(|error| error.to_string())?;
    let _required_audio_health = audio
        .internal_dac_health()
        .ok_or_else(|| "Orange internal DAC health monitor was not installed".to_string())?;
    if handoff_mode == crate::boot_oled_handoff::HandoffMode::V1 {
        return handoff::run(
            audio,
            usb_config,
            midi_rx,
            midi_handler,
            candidate_readiness,
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
    let render_result = render_shutdown(&render);
    let seesaw_result = seesaw.shutdown();
    result.and(render_result).and(seesaw_result)
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
) -> Result<(), String> {
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_prepared_runtime(
    prepared: PreparedRuntime,
    seesaw: &SeesawIo,
    encoder_rx: &Receiver<HardwareEvent>,
    render: &RenderWorker,
    audio_manager: &mut AudioManager,
    candidate_readiness: &mut CandidateReadiness,
    midi_rx: Receiver<MidiMessage>,
    initial_rendered: bool,
) -> Result<(), String> {
    let PreparedRuntime {
        mut playback,
        mut runner,
        mut host,
    } = prepared;
    let mut last_published_revision = 0;
    let mut readiness_gate = OrangeStartupReadinessGate::new(initial_rendered);
    ensure_required_audio_health(audio_manager.required_dac_status())?;
    let result = (|| {
        drain_host_work(&mut playback, &mut runner, &mut host)?;
        let first_snapshot_rendered = if initial_rendered {
            last_published_revision = playback.last_snapshot_revision();
            true
        } else {
            let rendered = publish_snapshot(
                &mut playback,
                &runner,
                &mut host,
                render,
                &mut last_published_revision,
                true,
            )?;
            readiness_gate.acknowledge_initial_write(if rendered {
                Ok(())
            } else {
                Err("Orange initial snapshot was not acknowledged".into())
            })?;
            rendered
        };
        if !first_snapshot_rendered {
            return Err("Orange runtime did not produce a valid initial snapshot".into());
        }
        readiness_gate.try_mark_ready(audio_manager.required_dac_status(), candidate_readiness)?;

        let mut previous_tick = Instant::now();
        let mut next_render = previous_tick + RENDER_INTERVAL;
        while !INTERRUPTED.load(Ordering::SeqCst) {
            let now = Instant::now();
            let elapsed = now.saturating_duration_since(previous_tick);
            previous_tick = now;
            audio_manager.recover_audio_if_due();
            ensure_required_audio_health(audio_manager.required_dac_status())?;
            readiness_gate
                .try_mark_ready(audio_manager.required_dac_status(), candidate_readiness)?;
            drain_midi_messages(&midi_rx, &mut playback, &mut runner, &mut host);
            drain_host_work(&mut playback, &mut runner, &mut host)?;
            drain_inputs(seesaw, encoder_rx, &mut playback, &mut runner, &mut host)?;
            let output = playback.advance_duration_with_output(elapsed, &mut runner, &mut host)?;
            process_runtime_output(&mut playback, &mut runner, &mut host, output)?;
            ensure_required_audio_health(audio_manager.required_dac_status())?;
            drain_host_work(&mut playback, &mut runner, &mut host)?;
            if now >= next_render {
                playback.request_next_snapshot();
                let at_ppqn_pulse = playback
                    .last_status()
                    .map(|status| status.current_ppqn_pulse);
                dispatch(
                    &mut playback,
                    &mut runner,
                    &mut host,
                    HostMessage::TransportPulseStep {
                        pulses: 0,
                        source: SyncSource::Internal,
                        at_ppqn_pulse,
                        request_snapshot: Some(true),
                    },
                )?;
                next_render = now + RENDER_INTERVAL;
            }
            publish_snapshot(
                &mut playback,
                &runner,
                &mut host,
                render,
                &mut last_published_revision,
                false,
            )?;
            thread::sleep(RUNTIME_TICK.min(POLLING_INTERVAL));
        }
        Ok::<(), String>(())
    })();
    let silence = host
        .silence_internal_audio()
        .map_err(|error| error.to_string());
    result.and(silence)
}

fn ensure_required_audio_health(status: OrangeDacStatus) -> Result<(), String> {
    if status == OrangeDacStatus::Terminal {
        Err("Orange internal DAC audio stream faulted".into())
    } else {
        Ok(())
    }
}

fn initialize_host_state(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
) -> Result<(), String> {
    let output = playback.dispatch_runner_messages(
        vec![playback_runtime::RunnerMessage::PlatformEffects {
            effects: vec![
                playback_runtime::RuntimePlatformEffect::StoreLoadDefault,
                playback_runtime::RuntimePlatformEffect::MidiListOutputsRequest,
                playback_runtime::RuntimePlatformEffect::MidiListInputsRequest,
            ],
        }],
        runner,
        host,
    )?;
    process_runtime_output(playback, runner, host, output)?;
    Ok(())
}

fn drain_host_work(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
) -> Result<(), String> {
    let responses = runner.flush_deferred_menu_apply()?;
    if !responses.is_empty() {
        let output = playback.dispatch_runner_messages(responses, runner, host)?;
        process_runtime_output(playback, runner, host, output)?;
    }
    for follow_up in host.flush_due_default_save()? {
        dispatch(playback, runner, host, follow_up)?;
    }
    drain_host_results(playback, runner, host)
}

fn drain_host_results(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
) -> Result<(), String> {
    for result in host.drain_results(HOST_RESULT_BUDGET) {
        dispatch(playback, runner, host, result)?;
    }
    Ok(())
}

fn drain_inputs(
    seesaw: &SeesawIo,
    encoder_rx: &Receiver<HardwareEvent>,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
) -> Result<(), String> {
    for _ in 0..32 {
        let message = match seesaw.input_rx.try_recv() {
            Ok(message) => message,
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
        };
        dispatch(playback, runner, host, message)?;
    }
    for _ in 0..32 {
        let event = match encoder_rx.try_recv() {
            Ok(event) => event,
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
        };
        if let Some(message) = encoder_message(event) {
            dispatch(playback, runner, host, message)?;
        }
    }
    Ok(())
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

fn encoder_message(event: HardwareEvent) -> Option<HostMessage> {
    match event {
        HardwareEvent::EncoderTurn { id, delta } => Some(encoder_turn_message(id, delta)),
        HardwareEvent::EncoderPress { id } => Some(encoder_press_message(id)),
        HardwareEvent::EncoderRelease { .. } => None,
    }
}

fn dispatch(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
    message: HostMessage,
) -> Result<(), String> {
    let output = playback.dispatch(
        playback_runtime::RuntimeDispatchInput::HostMessage(message),
        runner,
        host,
    )?;
    process_runtime_output(playback, runner, host, output)?;
    Ok(())
}

pub(crate) fn process_runtime_output(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
    output: playback_runtime::RuntimeIngest,
) -> Result<(), String> {
    ingest_oled_messages(host, &output.messages);
    let fault = host
        .oled_frame_fault()
        .map(crate::oled_frame_cache::OledFrameCacheFault::into_runtime_fault);
    let fault_output = playback.report_oled_cache_fault(fault);
    ingest_oled_messages(host, &fault_output.messages);
    for follow_up in fault_output.follow_ups {
        dispatch(playback, runner, host, follow_up)?;
    }
    for follow_up in output.follow_ups {
        dispatch(playback, runner, host, follow_up)?;
    }
    Ok(())
}

fn ingest_oled_messages(
    host: &mut OrangeHostAdapter,
    messages: &[playback_runtime::RunnerMessage],
) {
    for message in messages {
        host.ingest_oled_frame(message);
        if let playback_runtime::RunnerMessage::Snapshot { snapshot } = message {
            host.accept_oled_frame_reference(snapshot);
        }
    }
}

fn publish_snapshot(
    playback: &mut PlaybackRuntime,
    runner: &NativeRunner,
    host: &mut OrangeHostAdapter,
    render: &RenderWorker,
    last_revision: &mut u64,
    wait_for_render: bool,
) -> Result<bool, String> {
    if playback.last_snapshot_revision() == *last_revision {
        return Ok(false);
    }
    let Some(snapshot) = playback.last_snapshot().cloned() else {
        return Ok(false);
    };
    if wait_for_render
        && (!is_normal_menu_snapshot(&snapshot) || !runner.is_canonical_menu_presentation())
    {
        return Err("Orange initial snapshot is not a canonical normal menu".into());
    }
    let oled = host.oled_publication_for_snapshot(&snapshot, wait_for_render)?;
    if wait_for_render {
        render.publish_acknowledged_snapshot(snapshot, oled)?;
    } else if !render.publish_snapshot(snapshot, oled) {
        return Err("Orange render worker rejected a snapshot".into());
    }
    *last_revision = playback.last_snapshot_revision();
    Ok(true)
}

fn render_shutdown(render: &RenderWorker) -> Result<(), String> {
    render.publish_shutdown()
}

#[cfg(unix)]
fn install_signal_handlers() -> Result<(), String> {
    unsafe {
        let handler = interrupt_handler as *const () as libc::sighandler_t;
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            if libc::signal(signal, handler) == libc::SIG_ERR {
                return Err(format!(
                    "could not install Orange shutdown handler for signal {signal}"
                ));
            }
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn install_signal_handlers() -> Result<(), String> {
    Err("Orange foreground candidate requires Unix signal handling".into())
}
#[cfg(unix)]
extern "C" fn interrupt_handler(_: libc::c_int) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

#[cfg(test)]
#[path = "orange_candidate_tests.rs"]
mod tests;
