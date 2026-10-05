use super::{HardwareRenderCache, HardwareRenderTargets};
use crate::oled_frame_cache::OledFramePublication;
use crate::render::{
    initial_snapshot_render_result, mark_handoff_failed_decision, render_leds_only,
    render_oled_and_leds_cached, render_snapshot_cached, select_snapshot_render,
    snapshot_requires_oled_ack, LatestPresentation, OledOwnershipState, SnapshotRenderDecision,
};
use crate::render_loop_queue::RenderState as QueueState;
use serde_json::Value;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

pub(super) fn submit_presentation_recording_frame(
    state: &Arc<(Mutex<QueueState>, Condvar)>,
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

pub(super) struct PresentationState {
    pub(super) cache: HardwareRenderCache,
    pub(super) latest: Option<LatestPresentation>,
    pub(super) ownership: OledOwnershipState,
    pub(super) hdmi_ready: bool,
}

impl Default for PresentationState {
    fn default() -> Self {
        Self {
            cache: HardwareRenderCache::default(),
            latest: None,
            ownership: OledOwnershipState::default(),
            hdmi_ready: true,
        }
    }
}

pub(super) fn publish_legacy_snapshot(
    state: &Arc<(Mutex<QueueState>, Condvar)>,
    targets: &mut HardwareRenderTargets,
    presentation: &mut PresentationState,
    snapshot: Value,
    oled: OledFramePublication,
    rendered_acks: Vec<std::sync::mpsc::Sender<Result<(), String>>>,
    animation_deadline: &mut Option<Instant>,
) -> bool {
    if let Some(previous) = presentation.latest.as_mut() {
        previous.complete_native(Err("native scene superseded by legacy presentation".into()));
    }
    presentation.latest = match crate::render::prepare_legacy_presentation(
        snapshot.clone(),
        oled,
        &presentation.cache,
    ) {
        Ok(presentation) => Some(presentation),
        Err(error) => {
            for ack in rendered_acks {
                let _ = ack.send(Err(error.clone()));
            }
            return true;
        }
    };

    let physical_oled = match presentation.latest.as_ref() {
        Some(LatestPresentation::Legacy { oled, .. }) => oled,
        _ => unreachable!(),
    };
    let require_oled_ack = snapshot_requires_oled_ack(rendered_acks.len());
    let initial_acknowledged_snapshot = require_oled_ack
        && state.0.lock().ok().is_some_and(|state| {
            state.acknowledged_snapshot_published && !state.acknowledged_snapshot_rendered
        });
    if initial_acknowledged_snapshot {
        presentation.hdmi_ready = false;
    }

    let mut full_render_result = None;
    let mut pending_rendered_acks = Some(rendered_acks);
    let oled_render_count_before = presentation.cache.oled_render_count();
    let fail_initial_oled_write =
        super::take_startup_oled_write_failure(state, initial_acknowledged_snapshot);
    *animation_deadline =
        select_snapshot_render(presentation.ownership, |decision| match decision {
            SnapshotRenderDecision::OledAndLeds => {
                if presentation.hdmi_ready && !require_oled_ack {
                    render_snapshot_cached(
                        targets,
                        &snapshot,
                        physical_oled,
                        &mut presentation.cache,
                    )
                } else {
                    let rendered_before = presentation.cache.oled_render_count();
                    let oled_deadline = if fail_initial_oled_write {
                        None
                    } else {
                        render_oled_and_leds_cached(
                            targets,
                            &snapshot,
                            physical_oled,
                            &mut presentation.cache,
                        )
                    };
                    let frame_written = !fail_initial_oled_write
                        && presentation.cache.oled_render_count() > rendered_before;
                    if frame_written && require_oled_ack {
                        if let Some(current) = presentation.latest.as_ref() {
                            submit_presentation_recording_frame(state, current);
                        }
                    }
                    let render_result =
                        initial_snapshot_render_result(require_oled_ack, frame_written);
                    if let Some(result) = render_result.as_ref() {
                        if result.is_ok() && require_oled_ack {
                            if let Ok(mut state) = state.0.lock() {
                                state.acknowledged_snapshot_rendered = true;
                                if initial_acknowledged_snapshot {
                                    state.startup_accepted_oled_frame = presentation
                                        .latest
                                        .as_ref()
                                        .and_then(LatestPresentation::shared_oled_frame);
                                }
                            }
                        }
                        if let Some(acks) = pending_rendered_acks.take() {
                            for ack in acks {
                                let _ = ack.send(result.clone());
                            }
                        }
                    }
                    full_render_result = render_result;
                    if presentation.hdmi_ready {
                        let hdmi_deadline = crate::render::retry_hdmi_if_due(
                            targets,
                            &snapshot,
                            &mut presentation.cache,
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
                    render_leds_only(targets, &snapshot, &mut presentation.cache, Instant::now());
                let hdmi_deadline = presentation.hdmi_ready.then(|| {
                    crate::render::retry_hdmi_if_due(
                        targets,
                        &snapshot,
                        &mut presentation.cache,
                        Instant::now(),
                    )
                });
                full_render_result = initial_snapshot_render_result(require_oled_ack, false);
                crate::render::next_deadline(leds_deadline, hdmi_deadline.flatten())
            }
        });

    if !require_oled_ack && presentation.cache.oled_render_count() > oled_render_count_before {
        if let Some(current) = presentation.latest.as_ref() {
            submit_presentation_recording_frame(state, current);
        }
    }
    if full_render_result
        .as_ref()
        .is_some_and(|result| result.is_err())
        && mark_handoff_failed_decision(presentation.ownership)
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
        if let Some(acks) = pending_rendered_acks.take() {
            for ack in acks {
                let _ = ack.send(render_result.clone());
            }
        }
    } else if let Some(acks) = pending_rendered_acks {
        for ack in acks {
            let _ = ack.send(Ok(()));
        }
    }
    false
}
