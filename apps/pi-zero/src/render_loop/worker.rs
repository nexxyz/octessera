use super::presentation::{self, PresentationState};
use super::{ownership, terminal};
use crate::oled_frame_cache::OledFramePublication;
use crate::render::{
    mark_handoff_failed_decision, prepare_native_scene, retry_oled_decision, HardwareRenderTargets,
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
    let mut presentation = PresentationState::default();
    let mut animation_deadline = None;
    loop {
        let command = take_next_command(&state, animation_deadline);
        match command {
            Some(RenderCommand::Snapshot {
                snapshot,
                oled,
                rendered_acks,
            }) => {
                if presentation::publish_legacy_snapshot(
                    &state,
                    targets,
                    &mut presentation,
                    snapshot,
                    oled,
                    rendered_acks,
                    &mut animation_deadline,
                ) {
                    continue;
                }
            }
            Some(RenderCommand::NativeSnapshot(scene)) => {
                #[cfg(test)]
                super::native_worker_tests::wait_for_test_native_render_gate(&state);
                if let Some(mut next) = prepare_native_scene(scene, &presentation.cache) {
                    if let Some(previous) = presentation.latest.as_mut() {
                        previous.complete_native(Err("native scene superseded".into()));
                    }
                    let before = presentation.cache.oled_render_count();
                    let now = Instant::now();
                    animation_deadline =
                        crate::render::select_snapshot_render(presentation.ownership, |decision| {
                            match decision {
                                crate::render::SnapshotRenderDecision::OledAndLeds => {
                                    next.render_oled_leds(targets, &mut presentation.cache, now)
                                }
                                crate::render::SnapshotRenderDecision::LedsOnly => {
                                    next.render_leds(targets, &mut presentation.cache, now)
                                }
                            }
                        });
                    if presentation.cache.oled_render_count() > before {
                        presentation::submit_presentation_recording_frame(&state, &next);
                    }
                    if next.oled_is_accepted(&presentation.cache) {
                        next.complete_native(Ok(()));
                    }
                    if presentation.hdmi_ready {
                        let hdmi = next.render_hdmi(targets, &mut presentation.cache, now);
                        animation_deadline = crate::render::next_deadline(animation_deadline, hdmi);
                    }
                    presentation.latest = Some(next);
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
                    presentation.hdmi_ready = true;
                    animation_deadline =
                        crate::render::next_deadline(animation_deadline, Some(Instant::now()));
                }
            }
            Some(RenderCommand::MarkFailed { ack }) => {
                let result = if mark_handoff_failed_decision(presentation.ownership) {
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
                if ownership::handle_ownership_command(
                    &state,
                    targets,
                    &mut presentation,
                    stage,
                    cancellation,
                    ack,
                    &mut animation_deadline,
                ) {
                    continue;
                }
            }
            Some(RenderCommand::Shutdown { ack }) => {
                if let Some(current) = presentation.latest.as_mut() {
                    current.complete_native(Err("worker shutdown".into()));
                }
                let result = terminal::handle_shutdown(
                    targets,
                    &mut presentation.cache,
                    &presentation.latest,
                    &mut presentation.ownership,
                );
                let _ = ack.send(result);
                break;
            }
            Some(RenderCommand::PreserveTerminal {
                snapshot,
                oled,
                ack,
            }) => {
                if let Some(current) = presentation.latest.as_mut() {
                    current.complete_native(Err(
                        "terminal presentation superseded native scene".into()
                    ));
                }
                let before = presentation.cache.oled_render_count();
                let result = crate::render::physical_oled_publication(&oled, &presentation.cache)
                    .and_then(|physical| {
                        terminal::handle_preserve_terminal(
                            targets,
                            &mut presentation.cache,
                            &presentation.latest,
                            &mut presentation.ownership,
                            &snapshot,
                            &physical,
                        )
                        .map(|()| physical)
                    });
                if presentation.cache.oled_render_count() > before {
                    if let Ok(physical) = &result {
                        submit_recording_oled_frame(&state, physical);
                    }
                }
                let result = result.map(|_| ());
                let _ = ack.send(result);
                break;
            }
            Some(RenderCommand::Abort { ack }) => {
                if let Some(current) = presentation.latest.as_mut() {
                    current.complete_native(Err("worker abort".into()));
                }
                let result = terminal::handle_abort(
                    targets,
                    &mut presentation.cache,
                    &presentation.latest,
                    &mut presentation.ownership,
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
                    let sleep_deadline = presentation.cache.render_sleep_tick(targets, now);
                    let oled_before = presentation.cache.oled_render_count();
                    let (oled_retry_deadline, oled_retry_attempted) =
                        if retry_oled_decision(presentation.ownership) {
                            crate::render::retry_oled_if_due(
                                &mut targets.oled,
                                &mut presentation.cache,
                                now,
                            )
                        } else {
                            (None, false)
                        };
                    let frame_written = presentation.cache.oled_render_count() > oled_before;
                    if frame_written {
                        if let Some(current) = presentation.latest.as_ref() {
                            presentation::submit_presentation_recording_frame(&state, current);
                        }
                    }
                    if oled_retry_attempted {
                        if let Some(current) = presentation.latest.as_mut() {
                            if current.oled_is_accepted(&presentation.cache) {
                                if !frame_written {
                                    presentation::submit_presentation_recording_frame(
                                        &state, current,
                                    );
                                }
                                current.complete_native(Ok(()));
                            }
                        }
                    }
                    let hdmi_retry_deadline = presentation.hdmi_ready.then(|| {
                        presentation.latest.as_ref().and_then(|current| {
                            current.render_hdmi(targets, &mut presentation.cache, now)
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
