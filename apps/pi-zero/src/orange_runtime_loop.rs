use super::*;

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
    let audio = host.audio_service();
    let initial_published_revision = if initial_rendered {
        playback.last_snapshot_revision()
    } else {
        0
    };
    let mut scheduler = HardwareRuntimeScheduler::new(Instant::now(), initial_published_revision);
    let mut readiness_gate = OrangeStartupReadinessGate::new(initial_rendered);
    let mut pending_encoder_turns = PendingEncoderTurns::default();
    audio_manager.report_runtime_terminal_diagnostics();
    ensure_required_audio_health(audio_manager.required_jack_runtime_status())?;
    audio.ensure_route_readiness()?;
    audio_manager.ensure_selected_routes()?;
    let result = (|| {
        let initial_audio_prep = wait_for_initial_audio_prep(&mut playback, &mut runner, &mut host);
        readiness_gate.acknowledge_initial_audio_prep(initial_audio_prep)?;
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
        readiness_gate.try_mark_ready(
            audio_manager.required_jack_runtime_status(),
            candidate_readiness,
        )?;
        while !signal::interrupted() {
            if host.shutdown_pending() {
                break;
            }
            audio_manager.recover_audio_if_due();
            audio_manager.report_runtime_terminal_diagnostics();
            ensure_required_audio_health(audio_manager.required_jack_runtime_status())?;
            audio.ensure_route_readiness()?;
            audio_manager.ensure_selected_routes()?;
            readiness_gate.try_mark_ready(
                audio_manager.required_jack_runtime_status(),
                candidate_readiness,
            )?;
            drain_midi_messages(&midi_rx, &mut playback, &mut runner, &mut host);
            if host.shutdown_pending() {
                break;
            }
            drain_host_work(&mut playback, &mut runner, &mut host)?;
            if host.shutdown_pending() {
                break;
            }
            drain_inputs(
                seesaw,
                encoder_rx,
                &mut pending_encoder_turns,
                &mut playback,
                &mut runner,
                &mut host,
            )?;
            if host.shutdown_pending() {
                break;
            }
            let (runtime_snapshot_requested, runtime_advanced) =
                if let Some(advance) = scheduler.next_runtime_advance(Instant::now(), &playback) {
                    let revision_before = playback.last_snapshot_revision();
                    if advance.request_snapshot {
                        playback.request_next_snapshot();
                    }
                    let output = playback.advance_duration_with_output(
                        advance.elapsed,
                        &mut runner,
                        &mut host,
                    )?;
                    process_runtime_output(&mut playback, &mut runner, &mut host, output)?;
                    let revision_after = playback.last_snapshot_revision();
                    let completed_at = Instant::now();
                    if advance.request_snapshot {
                        scheduler.record_snapshot_attempt(
                            completed_at,
                            DisplaySnapshotDue::default(),
                            revision_before,
                            revision_after,
                        );
                    } else {
                        scheduler.observe_snapshot_revision(
                            completed_at,
                            revision_before,
                            revision_after,
                        );
                    }
                    (advance.request_snapshot, true)
                } else {
                    scheduler.observe_snapshot(Instant::now(), &playback);
                    (false, false)
                };
            if host.shutdown_pending() {
                break;
            }
            audio_manager.report_runtime_terminal_diagnostics();
            ensure_required_audio_health(audio_manager.required_jack_runtime_status())?;
            drain_host_work(&mut playback, &mut runner, &mut host)?;
            if runtime_advanced {
                scheduler.record_runtime_advance_complete(Instant::now(), &playback);
            }
            if host.shutdown_pending() {
                break;
            }
            let display_now = Instant::now();
            let display_due = scheduler.display_snapshot_due(display_now, &runner);
            if !runtime_snapshot_requested && display_due.any() {
                let message = scheduler.display_snapshot_message(&playback);
                let revision_before = playback.last_snapshot_revision();
                let dispatch_result = dispatch(&mut playback, &mut runner, &mut host, message);
                let revision_after = playback.last_snapshot_revision();
                scheduler.record_snapshot_attempt(
                    display_now,
                    display_due,
                    revision_before,
                    revision_after,
                );
                dispatch_result?;
            }
            let publish_now = Instant::now();
            if scheduler.snapshot_publication_due(publish_now, &playback) {
                publish_snapshot(
                    &mut playback,
                    &runner,
                    &mut host,
                    render,
                    &mut scheduler,
                    false,
                )?;
            }
            std::thread::sleep(scheduler.sleep_duration(Instant::now(), &playback, &runner));
        }
        Ok::<(), String>(())
    })();
    match (result, host.take_shutdown_request()) {
        (
            Ok(()),
            Some(
                request @ (crate::orange_device_apply::OrangeShutdownRequest::Reboot
                | crate::orange_device_apply::OrangeShutdownRequest::Shutdown),
            ),
        ) => {
            let action = match request {
                crate::orange_device_apply::OrangeShutdownRequest::Reboot => PowerAction::Reboot,
                crate::orange_device_apply::OrangeShutdownRequest::Shutdown => {
                    PowerAction::Shutdown
                }
                crate::orange_device_apply::OrangeShutdownRequest::ApplyDeviceConfig(_) => {
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
        (
            Err(error),
            Some(request @ crate::orange_device_apply::OrangeShutdownRequest::ApplyDeviceConfig(_)),
        ) => Err(crate::orange_device_apply::abort_shutdown_request(
            request, error, &mut host,
        )),
        (Err(error), Some(_ordinary_request)) => Err(OrangeRunError::Ordinary(error)),
        (Err(error), None) => {
            let _ = host.silence_internal_audio();
            Err(OrangeRunError::Ordinary(error))
        }
    }
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
    if host.shutdown_pending() {
        return Ok(());
    }
    for follow_up in host.flush_due_default_save()? {
        dispatch(playback, runner, host, follow_up)?;
    }
    drain_host_results(playback, runner, host)
}

pub(crate) fn drain_host_results(
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
    pending_encoder_turns: &mut PendingEncoderTurns,
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
    crate::encoder_queue::drain_encoder_events(
        encoder_rx,
        pending_encoder_turns,
        |message| dispatch(playback, runner, host, message),
        |_| {},
    )?;
    Ok(())
}

pub(crate) fn dispatch(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
    message: HostMessage,
) -> Result<(), String> {
    if host.shutdown_pending() {
        return Ok(());
    }
    let dispatch_input = host.handle_transfer_input(&message);
    while let Some(status) = host.take_transfer_status() {
        let output = playback.dispatch(
            playback_runtime::RuntimeDispatchInput::HostMessage(status),
            runner,
            host,
        )?;
        process_runtime_output(playback, runner, host, output)?;
    }
    if !dispatch_input {
        return Ok(());
    }
    let message = prepare_dispatch_message(playback, message);
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
        if host.shutdown_pending() {
            break;
        }
        dispatch(playback, runner, host, follow_up)?;
    }
    for follow_up in output.follow_ups {
        if host.shutdown_pending() {
            break;
        }
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
    let oled = match host.oled_publication_for_snapshot(&snapshot, wait_for_render) {
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
