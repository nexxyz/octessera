use super::*;
use crate::main_runtime_loop::{run_runtime_loop, BoardLoop, LoopInputs, LoopState};

#[cfg(test)]
#[path = "orange_runtime_error_tests.rs"]
mod error_tests;

#[cfg(test)]
#[path = "orange_runtime_keyboard_tests.rs"]
mod keyboard_tests;
#[cfg(test)]
#[path = "orange_timing_input_tests.rs"]
mod timing_input_tests;

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
) -> Result<crate::orange_device_apply::OrangeShutdownResolution, OrangeRunError> {
    let PreparedRuntime {
        mut playback,
        mut runner,
        mut host,
    } = prepared;
    let audio = host
        .audio_service()
        .expect("Orange host always owns its audio service");
    let initial_published_revision = if initial_rendered {
        playback.last_snapshot_revision()
    } else {
        0
    };
    let mut scheduler = HardwareRuntimeScheduler::new(Instant::now(), initial_published_revision);
    let mut readiness_gate = OrangeStartupReadinessGate::new(initial_rendered);
    audio_manager.report_runtime_terminal_diagnostics();
    ensure_required_audio_health(audio_manager.required_jack_runtime_status())?;
    audio.ensure_route_readiness()?;
    audio_manager.ensure_selected_routes()?;
    let result = (|| {
        let initial_audio_prep = wait_for_initial_audio_prep(&mut playback, &mut runner, &mut host);
        readiness_gate.acknowledge_initial_audio_prep(initial_audio_prep)?;
        let metrics = audio_manager.drain_audio_load_status(&mut playback);
        process_runtime_output(&mut playback, &mut runner, &mut host, metrics)?;
        scheduler.observe_snapshot(Instant::now(), &playback);
        let first_snapshot_rendered = if initial_rendered {
            true
        } else {
            let rendered = publish_snapshot(
                &mut playback,
                &runner,
                &mut host,
                render,
                &mut scheduler,
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
        audio_manager.report_runtime_terminal_diagnostics();
        ensure_required_audio_health(audio_manager.required_jack_runtime_status())?;
        audio.ensure_route_readiness()?;
        audio_manager.ensure_selected_routes()?;
        let timing_autoaux = crate::timing_input::validate_startup(&playback)?;
        readiness_gate.try_mark_ready(
            audio_manager.required_jack_runtime_status(),
            candidate_readiness,
        )?;
        let timing_input = crate::timing_input::TimingInput::prepare(
            timing_autoaux,
            &mut playback,
            &mut runner,
            &mut host,
        )?;
        let mut state = LoopState::new(scheduler, timing_input);
        let mut board = OrangeBoardLoop {
            audio_manager: &mut *audio_manager,
            audio: &audio,
            readiness_gate: &mut readiness_gate,
            candidate_readiness: &mut *candidate_readiness,
        };
        let inputs = LoopInputs {
            midi_rx: &midi_rx,
            input_rx: &seesaw.input_rx,
            encoder_rx,
        };
        run_runtime_loop(
            &mut state,
            &mut playback,
            &mut runner,
            &mut host,
            render,
            &inputs,
            &mut board,
        )?;
        Ok::<(), String>(())
    })();
    match (result, host.take_power_request()) {
        (
            Ok(()),
            Some(
                request @ (crate::host_adapter::PowerRequest::Reboot
                | crate::host_adapter::PowerRequest::Shutdown),
            ),
        ) => {
            let action = match request {
                crate::host_adapter::PowerRequest::Reboot => PowerAction::Reboot,
                crate::host_adapter::PowerRequest::Shutdown => PowerAction::Shutdown,
                crate::host_adapter::PowerRequest::ApplyDeviceConfig(_) => {
                    unreachable!("ordinary power branch excludes device apply")
                }
            };
            match lifecycle::run_ordinary_power_lifecycle(&playback, &mut host, render, action) {
                PowerLifecycleResult::Submitted => {
                    Ok(crate::orange_device_apply::OrangeShutdownResolution::Complete)
                }
                PowerLifecycleResult::Failed(failure) => {
                    eprintln!("Orange power lifecycle failed: {failure}");
                    Err(OrangeRunError::Ordinary(failure.to_string()))
                }
                PowerLifecycleResult::Duplicate => Err(OrangeRunError::Ordinary(
                    "Orange power lifecycle rejected a duplicate request".into(),
                )),
            }
        }
        (Ok(()), Some(request)) => {
            crate::orange_device_apply::resolve_shutdown_request(request, &mut host)
        }
        (Ok(()), None) => host
            .silence_internal_audio()
            .map(|_| crate::orange_device_apply::OrangeShutdownResolution::Complete)
            .map_err(|error| OrangeRunError::Ordinary(error.to_string())),
        (Err(error), Some(request @ crate::host_adapter::PowerRequest::ApplyDeviceConfig(_))) => {
            Err(crate::orange_device_apply::abort_shutdown_request(
                request, error, &mut host,
            ))
        }
        (Err(error), Some(_ordinary_request)) => Err(OrangeRunError::Ordinary(error)),
        (Err(error), None) => {
            let _ = host.silence_internal_audio();
            Err(OrangeRunError::Ordinary(error))
        }
    }
}

fn publish_snapshot(
    playback: &mut PlaybackRuntime,
    runner: &NativeRunner,
    host: &mut PiHostAdapter,
    render: &RenderWorker,
    scheduler: &mut HardwareRuntimeScheduler,
    wait_for_render: bool,
) -> Result<bool, String> {
    let snapshot_revision = playback.last_snapshot_revision();
    if snapshot_revision == scheduler.published_snapshot_revision() {
        return Ok(false);
    }
    scheduler.record_snapshot_publication_attempt(Instant::now());
    let Some(snapshot) = playback.last_snapshot().cloned() else {
        return Ok(false);
    };
    if wait_for_render
        && (!is_normal_menu_snapshot(&snapshot) || !runner.is_canonical_menu_presentation())
    {
        return Err("Orange initial snapshot is not a canonical normal menu".into());
    }
    let oled = match host
        .core
        .oled_publication_for_snapshot(&snapshot, wait_for_render)
    {
        Ok(oled) => oled,
        Err(error) if wait_for_render => return Err(error),
        Err(error) => {
            eprintln!("Orange OLED publication unavailable: {error}");
            return Ok(false);
        }
    };
    if wait_for_render {
        render.publish_acknowledged_snapshot(snapshot, oled)?;
        scheduler.record_snapshot_publication_accepted(snapshot_revision);
    } else {
        let accepted = render.publish_snapshot(snapshot, oled);
        if !accepted {
            return Ok(false);
        }
        scheduler.record_snapshot_publication_accepted(snapshot_revision);
    }
    Ok(true)
}

struct OrangeBoardLoop<'a> {
    audio_manager: &'a mut AudioManager,
    audio: &'a AudioService,
    readiness_gate: &'a mut OrangeStartupReadinessGate,
    candidate_readiness: &'a mut CandidateReadiness,
}

impl BoardLoop for OrangeBoardLoop<'_> {
    fn service_audio(
        &mut self,
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
        adapter: &mut PiHostAdapter,
    ) -> Result<(), String> {
        self.audio_manager.recover_audio_if_due();
        let metrics = self.audio_manager.drain_audio_load_status(playback);
        process_runtime_output(playback, runner, adapter, metrics)?;
        self.audio_manager.report_runtime_terminal_diagnostics();
        ensure_required_audio_health(self.audio_manager.required_jack_runtime_status())?;
        self.audio.ensure_route_readiness()?;
        self.audio_manager.ensure_selected_routes()?;
        self.readiness_gate.try_mark_ready(
            self.audio_manager.required_jack_runtime_status(),
            self.candidate_readiness,
        )
    }

    fn handle_power_request(
        &mut self,
        _playback: &PlaybackRuntime,
        adapter: &mut PiHostAdapter,
        _render_worker: &RenderWorker,
    ) -> bool {
        adapter.shutdown_pending()
    }

    fn interrupted(&self) -> bool {
        signal::interrupted()
    }
}
