use super::{ownership_command_cancelled, terminal};
use crate::render::{
    initial_snapshot_render_result, mark_handoff_failed_decision, ownership_stage_for_render,
    render_leds_only, render_oled_and_leds_cached, render_snapshot_cached,
    restore_after_dropped_ack_for_render, retry_oled_decision, select_snapshot_render,
    snapshot_requires_oled_ack, HardwareRenderCache, HardwareRenderTargets, OledOwnershipStage,
    OledOwnershipState, SnapshotRenderDecision,
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
    let mut latest_snapshot = None;
    let mut latest_oled = None;
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
                latest_snapshot = Some(snapshot.clone());
                latest_oled = Some(oled.clone());
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
                animation_deadline = select_snapshot_render(ownership, |decision| match decision {
                    SnapshotRenderDecision::OledAndLeds => {
                        if hdmi_ready && !require_oled_ack {
                            render_snapshot_cached(targets, &snapshot, &oled, &mut cache)
                        } else {
                            let rendered_before =
                                require_oled_ack.then(|| cache.oled_render_count());
                            let oled_deadline =
                                render_oled_and_leds_cached(targets, &snapshot, &oled, &mut cache);
                            let render_result = initial_snapshot_render_result(
                                require_oled_ack,
                                rendered_before
                                    .is_some_and(|before| cache.oled_render_count() > before),
                            );
                            if let Some(result) = render_result.as_ref() {
                                if result.is_ok() && require_oled_ack {
                                    if let Ok(mut state) = state.0.lock() {
                                        state.acknowledged_snapshot_rendered = true;
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
                    if let Some(SnapshotCommand {
                        snapshot,
                        oled,
                        rendered_acks,
                    }) = state
                        .0
                        .lock()
                        .ok()
                        .and_then(|mut state| state.snapshot.take())
                    {
                        latest_snapshot = Some(snapshot.clone());
                        latest_oled = Some(oled);
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
                        animation_deadline =
                            crate::render::next_deadline(leds_deadline, hdmi_deadline.flatten());
                        for ack in rendered_acks {
                            let _ = ack.send(Ok(()));
                        }
                    }
                }
                let result = if cancelled {
                    Err("OLED ownership command was cancelled".into())
                } else {
                    ownership_stage_for_render(
                        stage,
                        targets,
                        &mut cache,
                        &latest_snapshot,
                        &latest_oled,
                        &mut ownership,
                    )
                };
                if let Err(error) = restore_after_dropped_ack_for_render(
                    ack.send(result).is_err(),
                    targets,
                    &mut cache,
                    &latest_snapshot,
                    &latest_oled,
                    &mut ownership,
                ) {
                    eprintln!(
                        "OLED ownership rollback after dropped acknowledgement failed: {error}"
                    );
                }
            }
            Some(RenderCommand::Shutdown { ack }) => {
                let result = terminal::handle_shutdown(
                    targets,
                    &mut cache,
                    &latest_snapshot,
                    &latest_oled,
                    &mut ownership,
                );
                let _ = ack.send(result);
                break;
            }
            Some(RenderCommand::PreserveTerminal {
                snapshot,
                oled,
                ack,
            }) => {
                let result = terminal::handle_preserve_terminal(
                    targets,
                    &mut cache,
                    &latest_snapshot,
                    &latest_oled,
                    &mut ownership,
                    &snapshot,
                    &oled,
                );
                let _ = ack.send(result);
                break;
            }
            Some(RenderCommand::Abort { ack }) => {
                let result = terminal::handle_abort(
                    targets,
                    &mut cache,
                    &latest_snapshot,
                    &latest_oled,
                    &mut ownership,
                );
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
                    let oled_retry_deadline = if retry_oled_decision(ownership) {
                        crate::render::retry_oled_if_due(&mut targets.oled, &mut cache, now)
                    } else {
                        None
                    };
                    let hdmi_retry_deadline = hdmi_ready.then(|| {
                        latest_snapshot.as_ref().and_then(|snapshot| {
                            crate::render::retry_hdmi_if_due(targets, snapshot, &mut cache, now)
                        })
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
            return Some(RenderCommand::Snapshot {
                snapshot: snapshot.snapshot,
                oled: snapshot.oled,
                rendered_acks: snapshot.rendered_acks,
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
