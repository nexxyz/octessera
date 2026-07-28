use crate::audio::{AudioManager, AudioService};
use crate::audio_stream_health::AudioStreamHealth;
use crate::input::{encoder_press_message, encoder_turn_message};
use crate::orange_audio::{orange_samples_dir, OrangeAudioHost};
use crate::render::HardwareRenderTargets;
use crate::render_loop::RenderWorker;
use crate::seesaw_io::{self, SeesawIo};
use octessera_hal::board_profiles::{SeesawInputMode, ORANGE_PI_ZERO_2W_DEVICES};
use octessera_hal::encoder_gpio::HardwareEvent;
use octessera_hal::{NeoKey, NeoTrellis, OledSsd1351, OrangeEncoderGpio};
use playback_runtime::{
    HostAdapter, HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RuntimeConfig,
    SyncSource,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

const POLLING_INTERVAL: Duration = Duration::from_millis(10);
const RENDER_INTERVAL: Duration = Duration::from_millis(33);
const RUNTIME_TICK: Duration = Duration::from_millis(8);
const AUDIO_RESULT_BUDGET: usize = 4;

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

pub fn run() -> Result<(), String> {
    install_signal_handlers()?;
    let audio = AudioManager::new_orange(None)?;
    let required_audio_health = audio
        .internal_dac_health()
        .ok_or_else(|| "Orange internal DAC health monitor was not installed".to_string())?;
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
    let seesaw = seesaw_io::spawn_polling(trellis, neokey);
    let render = RenderWorker::spawn(HardwareRenderTargets {
        oled,
        seesaw_tx: seesaw.command_tx.clone(),
    });

    let audio_service = audio.service();
    let result = run_runtime(
        &seesaw,
        &encoder_rx,
        &render,
        &audio_service,
        &required_audio_health,
    );
    let render_result = render_shutdown(&render);
    let seesaw_result = seesaw.shutdown();
    result.and(render_result).and(seesaw_result)
}

fn run_runtime(
    seesaw: &SeesawIo,
    encoder_rx: &Receiver<HardwareEvent>,
    render: &RenderWorker,
    audio: &AudioService,
    required_audio_health: &AudioStreamHealth,
) -> Result<(), String> {
    let mut playback = PlaybackRuntime::new(RuntimeConfig {
        bpm: 120.0,
        sync_source: SyncSource::Internal,
        midi_clock_out_enabled: false,
        midi_out_enabled: false,
    });
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })?;
    let mut host = OrangeAudioHost::new(audio.clone(), orange_samples_dir()?);
    let mut last_published_revision = 0;
    let result = (|| {
        dispatch(
            &mut playback,
            &mut runner,
            &mut host,
            HostMessage::TransportPulseStep {
                pulses: 0,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(true),
            },
        )?;
        publish_snapshot(&mut playback, render, &mut last_published_revision)?;

        let mut previous_tick = Instant::now();
        let mut next_render = previous_tick + RENDER_INTERVAL;
        while !INTERRUPTED.load(Ordering::SeqCst) {
            let now = Instant::now();
            let elapsed = now.saturating_duration_since(previous_tick);
            previous_tick = now;
            ensure_required_audio_health(required_audio_health)?;
            drain_audio_results(&mut playback, &mut runner, &mut host, audio)?;
            drain_inputs(seesaw, encoder_rx, &mut playback, &mut runner, &mut host)?;
            playback.advance_duration(elapsed, &mut runner, &mut host)?;
            ensure_required_audio_health(required_audio_health)?;
            drain_audio_results(&mut playback, &mut runner, &mut host, audio)?;
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
            publish_snapshot(&mut playback, render, &mut last_published_revision)?;
            thread::sleep(RUNTIME_TICK.min(POLLING_INTERVAL));
        }
        Ok::<(), String>(())
    })();
    let silence = host
        .silence_internal_audio()
        .map_err(|error| error.to_string());
    result.and(silence)
}

fn ensure_required_audio_health(health: &AudioStreamHealth) -> Result<(), String> {
    if health.is_faulted() {
        Err("Orange internal DAC audio stream faulted".into())
    } else {
        Ok(())
    }
}

fn drain_audio_results(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeAudioHost,
    audio: &AudioService,
) -> Result<(), String> {
    for result in audio.drain_prep_results(AUDIO_RESULT_BUDGET) {
        dispatch(playback, runner, host, result)?;
    }
    Ok(())
}

fn drain_inputs(
    seesaw: &SeesawIo,
    encoder_rx: &Receiver<HardwareEvent>,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeAudioHost,
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
        if !pins.is_qualified() {
            let conflict = pins
                .uart_conflict
                .expect("unqualified Orange encoder must have a recorded conflict");
            eprintln!(
                "{id} encoder excluded: physical pin {} / GPIO offset {} conflicts with active {}",
                conflict.physical_pin, conflict.offset, conflict.signal
            );
            continue;
        }
        encoders.push(OrangeEncoderGpio::new(id, pins, event_tx.clone())?);
    }
    Ok((event_rx, encoders))
}

#[cfg(test)]
fn qualified_encoder_ids() -> Vec<&'static str> {
    ORANGE_PI_ZERO_2W_DEVICES
        .encoders
        .iter()
        .enumerate()
        .filter(|(_, pins)| pins.is_qualified())
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
    host: &mut OrangeAudioHost,
    message: HostMessage,
) -> Result<(), String> {
    let output = playback.dispatch(
        playback_runtime::RuntimeDispatchInput::HostMessage(message),
        runner,
        host,
    )?;
    for follow_up in output.follow_ups {
        dispatch(playback, runner, host, follow_up)?;
    }
    Ok(())
}

fn publish_snapshot(
    playback: &mut PlaybackRuntime,
    render: &RenderWorker,
    last_revision: &mut u64,
) -> Result<(), String> {
    if playback.last_snapshot_revision() == *last_revision {
        return Ok(());
    }
    let Some(snapshot) = playback.last_snapshot().cloned() else {
        return Ok(());
    };
    let pulses = playback.drain_ui_pulses();
    if !render.publish_snapshot(snapshot, pulses) {
        return Err("Orange render worker rejected a snapshot".into());
    }
    *last_revision = playback.last_snapshot_revision();
    Ok(())
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
mod tests {
    use super::{
        drain_audio_results, encoder_id, encoder_message, qualified_encoder_ids, POLLING_INTERVAL,
        RENDER_INTERVAL, RUNTIME_TICK,
    };
    use crate::audio::test_service_with_prep_sender;
    use crate::orange_audio::OrangeAudioHost;
    use octessera_hal::encoder_gpio::HardwareEvent;
    use playback_runtime::{
        HostMessage, MusicalEvent, NativeRunner, NativeRunnerConfig, PlaybackRuntime,
        RunnerMessage, RuntimeConfig, RuntimeDispatchInput, RuntimeOperation, RuntimeStoreResult,
    };
    use std::path::PathBuf;

    #[test]
    fn orange_candidate_uses_ten_millisecond_polling() {
        assert_eq!(POLLING_INTERVAL.as_millis(), 10);
        assert!(RUNTIME_TICK <= POLLING_INTERVAL);
        assert!(RENDER_INTERVAL > POLLING_INTERVAL);
    }

    #[test]
    fn qualified_encoder_ids_preserve_main_and_aux_semantics() {
        assert_eq!(encoder_id(0), Ok("encoder_main"));
        assert_eq!(encoder_id(1), Ok("encoder_aux_1"));
        assert_eq!(encoder_id(2), Ok("encoder_aux_2"));
        assert_eq!(encoder_id(3), Ok("encoder_aux_3"));
        assert!(encoder_id(4).is_err());
    }

    #[test]
    fn orange_encoder_events_use_native_input_messages() {
        let HostMessage::DeviceInput { input, .. } = encoder_message(HardwareEvent::EncoderTurn {
            id: "encoder_main",
            delta: 1,
        })
        .unwrap() else {
            panic!("expected main encoder turn input");
        };
        assert_eq!(input["type"], "encoder_turn");
        assert_eq!(input["id"], "main");

        let HostMessage::DeviceInput { input, .. } = encoder_message(HardwareEvent::EncoderPress {
            id: "encoder_aux_2",
        })
        .unwrap() else {
            panic!("expected aux encoder press input");
        };
        assert_eq!(input["type"], "encoder_press");
        assert_eq!(input["id"], "aux2");
        assert!(encoder_message(HardwareEvent::EncoderRelease { id: "encoder_main" }).is_none());
    }

    #[test]
    fn orange_candidate_composes_only_non_uart_encoders() {
        assert_eq!(
            qualified_encoder_ids(),
            ["encoder_main", "encoder_aux_1", "encoder_aux_3"]
        );
    }

    #[test]
    fn orange_audio_prep_results_are_redispatched_to_runtime() {
        let (audio, _, _, result_tx) = test_service_with_prep_sender();
        result_tx
            .send(HostMessage::RuntimeResult {
                result: RuntimeStoreResult::OperationSucceeded {
                    operation: RuntimeOperation::AudioCommand,
                    request_id: None,
                    revision: Some(1),
                },
            })
            .unwrap();
        let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        let mut host = OrangeAudioHost::new(audio.clone(), PathBuf::from("samples"));

        drain_audio_results(&mut playback, &mut runner, &mut host, &audio).unwrap();

        assert!(audio.drain_prep_results(1).is_empty());
    }

    #[test]
    fn orange_runner_midi_events_do_not_latch_midi_error_when_disabled() {
        let (audio, _, _) = crate::audio::test_service();
        let mut host = OrangeAudioHost::new(audio, PathBuf::from("samples"));
        let mut playback = PlaybackRuntime::new(RuntimeConfig {
            midi_out_enabled: true,
            ..RuntimeConfig::default()
        });
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

        playback
            .dispatch(
                RuntimeDispatchInput::RunnerMessages(vec![RunnerMessage::MidiEvents {
                    events: vec![MusicalEvent::NoteOn {
                        channel: 0,
                        note: 60,
                        velocity: 100,
                        duration_ms: Some(25),
                    }],
                }]),
                &mut runner,
                &mut host,
            )
            .unwrap();

        assert!(playback.latched_errors().is_empty());
    }

    #[test]
    fn required_internal_dac_fault_terminates_orange_runtime() {
        let health = crate::audio_stream_health::AudioStreamHealth::new("InternalDac".into());
        health.log(cpal::StreamError::DeviceNotAvailable);

        let error = super::ensure_required_audio_health(&health).unwrap_err();

        assert_eq!(error, "Orange internal DAC audio stream faulted");
    }
}
