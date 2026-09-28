use super::presentation::{self, PresentationState};
use super::HardwareRenderTargets;
use crate::render::{
    ownership_stage_for_render, prepare_native_scene, restore_after_dropped_ack_for_render,
    OledOwnershipStage,
};
use crate::render_loop_queue::{RenderState, SnapshotCommand};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::time::Instant;

pub(super) fn handle_ownership_command(
    state: &Arc<(Mutex<RenderState>, Condvar)>,
    targets: &mut HardwareRenderTargets,
    presentation: &mut PresentationState,
    stage: OledOwnershipStage,
    cancellation: Arc<std::sync::atomic::AtomicBool>,
    ack: mpsc::Sender<Result<(), String>>,
    animation_deadline: &mut Option<Instant>,
) -> bool {
    let cancelled = super::ownership_command_cancelled(&cancellation);
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
                    if let Some(previous) = presentation.latest.as_mut() {
                        previous.complete_native(Err(
                            "native scene superseded by legacy presentation".into(),
                        ));
                    }
                    presentation.latest = match crate::render::prepare_legacy_presentation(
                        snapshot,
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
                    let now = Instant::now();
                    let current = presentation.latest.as_ref().unwrap();
                    let leds = current.render_leds(targets, &mut presentation.cache, now);
                    let hdmi = presentation
                        .hdmi_ready
                        .then(|| current.render_hdmi(targets, &mut presentation.cache, now))
                        .flatten();
                    *animation_deadline = crate::render::next_deadline(leds, hdmi);
                    for ack in rendered_acks {
                        let _ = ack.send(Ok(()));
                    }
                }
                SnapshotCommand::Native(scene) => {
                    if let Some(next) = prepare_native_scene(scene, &presentation.cache) {
                        if let Some(previous) = presentation.latest.as_mut() {
                            previous.complete_native(Err("native scene superseded".into()));
                        }
                        presentation.latest = Some(next);
                        let now = Instant::now();
                        let current = presentation.latest.as_ref().unwrap();
                        let leds = current.render_leds(targets, &mut presentation.cache, now);
                        let hdmi = presentation
                            .hdmi_ready
                            .then(|| current.render_hdmi(targets, &mut presentation.cache, now))
                            .flatten();
                        *animation_deadline = crate::render::next_deadline(leds, hdmi);
                    }
                }
            }
        }
    }

    let oled_render_count_before = presentation.cache.oled_render_count();
    let result = if cancelled {
        Err("OLED ownership command was cancelled".into())
    } else {
        ownership_stage_for_render(
            stage,
            targets,
            &mut presentation.cache,
            &presentation.latest,
            &mut presentation.ownership,
        )
    };
    if stage == OledOwnershipStage::ResumeComplete {
        if let Some(current) = presentation.latest.as_mut() {
            if result.is_ok() && presentation.cache.oled_render_count() > oled_render_count_before {
                presentation::submit_presentation_recording_frame(state, current);
            }
            current.complete_native(result.clone());
        }
    }
    if let Err(error) = restore_after_dropped_ack_for_render(
        ack.send(result).is_err(),
        targets,
        &mut presentation.cache,
        &presentation.latest,
        &mut presentation.ownership,
    ) {
        eprintln!("OLED ownership rollback after dropped acknowledgement failed: {error}");
    }
    false
}
