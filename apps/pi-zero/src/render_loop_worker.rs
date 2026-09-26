use super::{ownership_command_cancelled, terminal};
use crate::oled_frame_cache::OledFramePublication;
use crate::render::{
    initial_snapshot_render_result, mark_handoff_failed_decision, ownership_stage_for_render,
    prepare_native_scene, render_leds_only, render_oled_and_leds_cached, render_snapshot_cached,
    restore_after_dropped_ack_for_render, retry_oled_decision, select_snapshot_render,
    snapshot_requires_oled_ack, HardwareRenderCache, HardwareRenderTargets, LatestPresentation,
    OledOwnershipStage, OledOwnershipState, SnapshotRenderDecision,
};
use crate::render_loop_queue::{
    pending_work_wins_over_expired_animation_deadline, RenderCommand, RenderState, SnapshotCommand,
};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

pub(super) fn render_worker_loop(
    state: Arc<(Mutex<RenderState>, Condvar)>,
    targets: &mut HardwareRenderTargets,
) {
    let mut cache = HardwareRenderCache::default();
    let mut animation_deadline = None;
    let mut latest: Option<LatestPresentation> = None;
    let mut ownership = OledOwnershipState::default();
    let mut hdmi_ready = true;
    loop {
        let command = take_next_command(&state, animation_deadline);
        match command {
            Some(RenderCommand::Snapshot {
                snapshot,
                oled,
                rendered_acks,
            }) => {
                if let Some(previous) = latest.as_mut() {
                    previous.complete_native(Err(
                        "native scene superseded by legacy presentation".into()
                    ));
                }
                latest = match crate::render::prepare_legacy_presentation(
                    snapshot.clone(),
                    oled,
                    &cache,
                ) {
                    Ok(presentation) => Some(presentation),
                    Err(error) => {
                        for ack in rendered_acks {
                            let _ = ack.send(Err(error.clone()));
                        }
                        continue;
                    }
                };
                let physical_oled = match latest.as_ref() {
                    Some(LatestPresentation::Legacy { oled, .. }) => oled,
                    _ => unreachable!(),
                };
                let require_oled_ack = snapshot_requires_oled_ack(rendered_acks.len());
                let initial_acknowledged_snapshot = require_oled_ack
                    && state.0.lock().ok().is_some_and(|state| {
                        state.acknowledged_snapshot_published
                            && !state.acknowledged_snapshot_rendered
                    });
                if initial_acknowledged_snapshot {
                    hdmi_ready = false;
                }
                let mut full_render_result = None;
                let mut pending_rendered_acks = Some(rendered_acks);
                let mut rendered_acks_sent = false;
                let oled_render_count_before = cache.oled_render_count();
                let fail_initial_oled_write =
                    super::take_startup_oled_write_failure(&state, initial_acknowledged_snapshot);
                animation_deadline = select_snapshot_render(ownership, |decision| match decision {
                    SnapshotRenderDecision::OledAndLeds => {
                        if hdmi_ready && !require_oled_ack {
                            render_snapshot_cached(targets, &snapshot, physical_oled, &mut cache)
                        } else {
                            let rendered_before = cache.oled_render_count();
                            let oled_deadline = if fail_initial_oled_write {
                                None
                            } else {
                                render_oled_and_leds_cached(
                                    targets,
                                    &snapshot,
                                    physical_oled,
                                    &mut cache,
                                )
                            };
                            let frame_written = !fail_initial_oled_write
                                && cache.oled_render_count() > rendered_before;
                            if frame_written && require_oled_ack {
                                if let Some(current) = latest.as_ref() {
                                    submit_presentation_recording_frame(&state, current);
                                }
                            }
                            let render_result =
                                initial_snapshot_render_result(require_oled_ack, frame_written);
                            if let Some(result) = render_result.as_ref() {
                                if result.is_ok() && require_oled_ack {
                                    if let Ok(mut state) = state.0.lock() {
                                        state.acknowledged_snapshot_rendered = true;
                                        if initial_acknowledged_snapshot {
                                            state.startup_accepted_oled_frame = latest
                                                .as_ref()
                                                .and_then(LatestPresentation::shared_oled_frame);
                                        }
                                    }
                                }
                                if let Some(rendered_acks) = pending_rendered_acks.take() {
                                    for ack in rendered_acks {
                                        let _ = ack.send(result.clone());
                                    }
                                    rendered_acks_sent = true;
                                }
                            }
                            full_render_result = render_result;
                            if hdmi_ready {
                                let hdmi_deadline = crate::render::retry_hdmi_if_due(
                                    targets,
                                    &snapshot,
                                    &mut cache,
                                    Instant::now(),
                                );
                                crate::render::next_deadline(oled_deadline, hdmi_deadline)
                            } else {
                                oled_deadline
                            }
                        }
                    }
                    SnapshotRenderDecision::LedsOnly => {
                        let leds_deadline =
                            render_leds_only(targets, &snapshot, &mut cache, Instant::now());
                        let hdmi_deadline = hdmi_ready.then(|| {
                            crate::render::retry_hdmi_if_due(
                                targets,
                                &snapshot,
                                &mut cache,
                                Instant::now(),
                            )
                        });
                        full_render_result =
                            initial_snapshot_render_result(require_oled_ack, false);
                        crate::render::next_deadline(leds_deadline, hdmi_deadline.flatten())
                    }
                });
                if !require_oled_ack && cache.oled_render_count() > oled_render_count_before {
                    if let Some(current) = latest.as_ref() {
                        submit_presentation_recording_frame(&state, current);
                    }
                }
                if full_render_result
                    .as_ref()
                    .is_some_and(|result| result.is_err())
                    && mark_handoff_failed_decision(ownership)
                {
                    if let Some(handoff) = targets.oled_handoff.as_ref() {
                        if let Err(error) = handoff.mark_failed_result() {
                            eprintln!(
                                "OLED handoff failure-state publication after initial render failed: {error}"
                            );
                        }
                    }
                }
                if let Some(render_result) = full_render_result {
                    if render_result.is_ok() && require_oled_ack {
                        if let Ok(mut state) = state.0.lock() {
                            state.acknowledged_snapshot_rendered = true;
                        }
                    }
                    if !rendered_acks_sent {
                        if let Some(rendered_acks) = pending_rendered_acks {
                            for ack in rendered_acks {
                                let _ = ack.send(render_result.clone());
                            }
                        }
                    }
                } else if let Some(rendered_acks) = pending_rendered_acks {
                    for ack in rendered_acks {
                        let _ = ack.send(Ok(()));
                    }
                }
            }
            Some(RenderCommand::NativeSnapshot(scene)) => {
                #[cfg(test)]
                super::native_worker_tests::wait_for_test_native_render_gate(&state);
                if let Some(mut next) = prepare_native_scene(scene, &cache) {
                    if let Some(previous) = latest.as_mut() {
                        previous.complete_native(Err("native scene superseded".into()));
                    }
                    let before = cache.oled_render_count();
                    let now = Instant::now();
                    animation_deadline =
                        select_snapshot_render(ownership, |decision| match decision {
                            SnapshotRenderDecision::OledAndLeds => {
                                next.render_oled_leds(targets, &mut cache, now)
                            }
                            SnapshotRenderDecision::LedsOnly => {
                                next.render_leds(targets, &mut cache, now)
                            }
                        });
                    if cache.oled_render_count() > before {
                        submit_presentation_recording_frame(&state, &next);
                    }
                    if next.oled_is_accepted(&cache) {
                        next.complete_native(Ok(()));
                    }
                    if hdmi_ready {
                        let hdmi = next.render_hdmi(targets, &mut cache, now);
                        animation_deadline = crate::render::next_deadline(animation_deadline, hdmi);
                    }
                    latest = Some(next);
                }
            }
            Some(RenderCommand::MarkFirstMenuRendered { ack }) => {
                let result = targets
                    .oled_handoff
                    .as_mut()
                    .map_or(Ok(()), |handoff| handoff.mark_first_menu_rendered());
                let hdmi_can_start = result.is_ok();
                let _ = ack.send(result);
                if hdmi_can_start {
                    hdmi_ready = true;
                    animation_deadline =
                        crate::render::next_deadline(animation_deadline, Some(Instant::now()));
                }
            }
            Some(RenderCommand::MarkFailed { ack }) => {
                let result = if mark_handoff_failed_decision(ownership) {
                    targets
                        .oled_handoff
                        .as_ref()
                        .map_or(Ok(()), |handoff| handoff.mark_failed_result())
                } else {
                    Ok(())
                };
                let _ = ack.send(result);
            }
            Some(RenderCommand::Ownership {
                stage,
                cancellation,
                ack,
            }) => {
                let cancelled = ownership_command_cancelled(&cancellation);
                if !cancelled && stage == OledOwnershipStage::ResumeComplete {
                    if let Some(pending) = state
                        .0
                        .lock()
                        .ok()
                        .and_then(|mut state| state.snapshot.take())
                    {
                        match pending {
                            SnapshotCommand::Legacy {
                                snapshot,
                                oled,
                                rendered_acks,
                            } => {
                                if let Some(previous) = latest.as_mut() {
                                    previous.complete_native(Err(
                                        "native scene superseded by legacy presentation".into(),
                                    ));
                                }
                                latest = match crate::render::prepare_legacy_presentation(
                                    snapshot, oled, &cache,
                                ) {
                                    Ok(presentation) => Some(presentation),
                                    Err(error) => {
                                        for ack in rendered_acks {
                                            let _ = ack.send(Err(error.clone()));
                                        }
                                        continue;
                                    }
                                };
                                let now = Instant::now();
                                let current = latest.as_ref().unwrap();
                                let leds = current.render_leds(targets, &mut cache, now);
                                let hdmi = hdmi_ready
                                    .then(|| current.render_hdmi(targets, &mut cache, now))
                                    .flatten();
                                animation_deadline = crate::render::next_deadline(leds, hdmi);
                                for ack in rendered_acks {
                                    let _ = ack.send(Ok(()));
                                }
                            }
                            SnapshotCommand::Native(scene) => {
                                if let Some(next) = prepare_native_scene(scene, &cache) {
                                    if let Some(previous) = latest.as_mut() {
                                        previous
                                            .complete_native(Err("native scene superseded".into()));
                                    }
                                    latest = Some(next);
                                    let now = Instant::now();
                                    let current = latest.as_ref().unwrap();
                                    let leds = current.render_leds(targets, &mut cache, now);
                                    let hdmi = hdmi_ready
                                        .then(|| current.render_hdmi(targets, &mut cache, now))
                                        .flatten();
                                    animation_deadline = crate::render::next_deadline(leds, hdmi);
                                }
                            }
                        }
                    }
                }
                let oled_render_count_before = cache.oled_render_count();
                let result = if cancelled {
                    Err("OLED ownership command was cancelled".into())
                } else {
                    ownership_stage_for_render(stage, targets, &mut cache, &latest, &mut ownership)
                };
                if stage == OledOwnershipStage::ResumeComplete {
                    if let Some(current) = latest.as_mut() {
                        if result.is_ok() && cache.oled_render_count() > oled_render_count_before {
                            submit_presentation_recording_frame(&state, current);
                        }
                        current.complete_native(result.clone());
                    }
                }
                if let Err(error) = restore_after_dropped_ack_for_render(
                    ack.send(result).is_err(),
                    targets,
                    &mut cache,
                    &latest,
                    &mut ownership,
                ) {
                    eprintln!(
                        "OLED ownership rollback after dropped acknowledgement failed: {error}"
                    );
                }
            }
            Some(RenderCommand::Shutdown { ack }) => {
                if let Some(current) = latest.as_mut() {
                    current.complete_native(Err("worker shutdown".into()));
                }
                let result =
                    terminal::handle_shutdown(targets, &mut cache, &latest, &mut ownership);
                let _ = ack.send(result);
                break;
            }
            Some(RenderCommand::PreserveTerminal {
                snapshot,
                oled,
                ack,
            }) => {
                if let Some(current) = latest.as_mut() {
                    current.complete_native(Err(
                        "terminal presentation superseded native scene".into()
                    ));
                }
                let before = cache.oled_render_count();
                let result =
                    crate::render::physical_oled_publication(&oled, &cache).and_then(|physical| {
                        terminal::handle_preserve_terminal(
                            targets,
                            &mut cache,
                            &latest,
                            &mut ownership,
                            &snapshot,
                            &physical,
                        )
                        .map(|()| physical)
                    });
                if cache.oled_render_count() > before {
                    if let Ok(physical) = &result {
                        submit_recording_oled_frame(&state, physical);
                    }
                }
                let result = result.map(|_| ());
                let _ = ack.send(result);
                break;
            }
            Some(RenderCommand::Abort { ack }) => {
                if let Some(current) = latest.as_mut() {
                    current.complete_native(Err("worker abort".into()));
                }
                let result = terminal::handle_abort(targets, &mut cache, &latest, &mut ownership);
                let _ = ack.send(result);
                break;
            }
            None => {
                let pending_work = {
                    let state = state.0.lock().expect("render worker state mutex poisoned");
                    pending_work_wins_over_expired_animation_deadline(&state)
                };
                if pending_work {
                    animation_deadline = None;
                } else {
                    let now = Instant::now();
                    let sleep_deadline = cache.render_sleep_tick(targets, now);
                    let oled_before = cache.oled_render_count();
                    let (oled_retry_deadline, oled_retry_attempted) =
                        if retry_oled_decision(ownership) {
                            crate::render::retry_oled_if_due(&mut targets.oled, &mut cache, now)
                        } else {
                            (None, false)
                        };
                    let frame_written = cache.oled_render_count() > oled_before;
                    if frame_written {
                        if let Some(current) = latest.as_ref() {
                            submit_presentation_recording_frame(&state, current);
                        }
                    }
                    if oled_retry_attempted {
                        if let Some(current) = latest.as_mut() {
                            if current.oled_is_accepted(&cache) {
                                if !frame_written {
                                    submit_presentation_recording_frame(&state, current);
                                }
                                current.complete_native(Ok(()));
                            }
                        }
                    }
                    let hdmi_retry_deadline = hdmi_ready.then(|| {
                        latest
                            .as_ref()
                            .and_then(|current| current.render_hdmi(targets, &mut cache, now))
                    });
                    animation_deadline = crate::render::next_deadline(
                        crate::render::next_deadline(sleep_deadline, oled_retry_deadline),
                        hdmi_retry_deadline.flatten(),
                    );
                }
            }
        }
    }
}

fn submit_presentation_recording_frame(
    state: &Arc<(Mutex<RenderState>, Condvar)>,
    presentation: &LatestPresentation,
) {
    let audio = state
        .0
        .lock()
        .ok()
        .and_then(|state| state.recording_audio.clone());
    if let Some(audio) = audio {
        if let Err(error) = presentation.submit_recording_frame(&audio) {
            eprintln!("native OLED recording submission failed: {error}");
        }
    }
}

fn submit_recording_oled_frame(
    state: &Arc<(Mutex<RenderState>, Condvar)>,
    publication: &OledFramePublication,
) {
    let audio = state
        .0
        .lock()
        .ok()
        .and_then(|state| state.recording_audio.clone());
    if let (Some(audio), Some(revision), Some(pixels)) =
        (audio, publication.revision(), publication.shared_pixels())
    {
        if let Err(error) = audio.submit_accepted_oled_frame_shared(revision, pixels) {
            eprintln!("terminal OLED recording submission failed: {error}");
        }
    }
}

pub(super) fn take_next_command(
    state: &Arc<(Mutex<RenderState>, Condvar)>,
    animation_deadline: Option<Instant>,
) -> Option<RenderCommand> {
    let (lock, ready) = &**state;
    let mut guard = lock.lock().expect("render worker state mutex poisoned");
    loop {
        if let Some(command) = guard.command.take() {
            return Some(command);
        }
        if let Some(snapshot) = guard.snapshot.take() {
            return Some(match snapshot {
                SnapshotCommand::Legacy {
                    snapshot,
                    oled,
                    rendered_acks,
                } => RenderCommand::Snapshot {
                    snapshot,
                    oled,
                    rendered_acks,
                },
                SnapshotCommand::Native(scene) => RenderCommand::NativeSnapshot(scene),
            });
        }
        let Some(deadline) = animation_deadline else {
            guard = ready
                .wait(guard)
                .expect("render worker state mutex poisoned while waiting");
            continue;
        };
        let timeout = deadline.saturating_duration_since(Instant::now());
        if timeout.is_zero() {
            return None;
        }
        let (next_guard, result) = ready
            .wait_timeout(guard, timeout)
            .expect("render worker state mutex poisoned while waiting");
        guard = next_guard;
        if result.timed_out() && !pending_work_wins_over_expired_animation_deadline(&guard) {
            return None;
        }
    }
}
