use super::{HardwareRenderCache, HardwareRenderTargets};
use crate::render::{
    mark_handoff_failed_decision, prepare_native_scene, select_scene_render, LatestPresentation,
    OledOwnershipState, SceneRenderDecision,
};
use crate::render_loop_queue::{NativeSceneCommand, RenderState as QueueState};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

pub(super) fn submit_presentation_recording_frame(
    state: &Arc<(Mutex<QueueState>, Condvar)>,
    presentation: &LatestPresentation,
) {
    if let Some(audio) = recording_audio(state) {
        if let Err(error) = presentation.submit_recording_frame(&audio) {
            eprintln!("native OLED recording submission failed: {error}");
        }
    }
}

pub(super) fn submit_presentation_recording_leds(
    state: &Arc<(Mutex<QueueState>, Condvar)>,
    presentation: &LatestPresentation,
) {
    if let Some(audio) = recording_audio(state) {
        if let Err(error) = presentation.submit_recording_leds(&audio) {
            eprintln!("native LED recording submission failed: {error}");
        }
    }
}

pub(super) fn recording_audio(
    state: &Arc<(Mutex<QueueState>, Condvar)>,
) -> Option<crate::audio::AudioService> {
    state
        .0
        .lock()
        .ok()
        .and_then(|state| state.recording_audio.clone())
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

pub(super) fn present_native_scene(
    state: &Arc<(Mutex<QueueState>, Condvar)>,
    targets: &mut HardwareRenderTargets,
    presentation: &mut PresentationState,
    scene: Box<NativeSceneCommand>,
    animation_deadline: &mut Option<Instant>,
) {
    let Some(mut next) = prepare_native_scene(scene, &presentation.cache) else {
        return;
    };
    if let Some(previous) = presentation.latest.as_mut() {
        previous.complete_native(Err("native scene superseded".into()));
    }
    let initial = state
        .0
        .lock()
        .ok()
        .is_some_and(|state| state.initial_scene_published && !state.initial_scene_rendered);
    if initial {
        presentation.hdmi_ready = false;
    }
    let fail_initial_write = super::take_startup_oled_write_failure(state, initial);
    let before = presentation.cache.oled_render_count();
    let now = Instant::now();
    *animation_deadline = select_scene_render(presentation.ownership, |decision| match decision {
        SceneRenderDecision::OledAndLeds if !fail_initial_write => {
            next.render_oled_leds(targets, &mut presentation.cache, now)
        }
        _ => next.render_leds(targets, &mut presentation.cache, now),
    });
    let frame_written = presentation.cache.oled_render_count() > before;
    if frame_written {
        submit_presentation_recording_frame(state, &next);
    }
    submit_presentation_recording_leds(state, &next);
    if initial {
        complete_initial_scene(
            state,
            targets,
            presentation.ownership,
            &mut next,
            frame_written,
        );
    } else if next.oled_is_accepted(&presentation.cache) {
        next.complete_native(Ok(()));
    }
    if presentation.hdmi_ready {
        let hdmi = next.render_hdmi(targets, &mut presentation.cache, now);
        *animation_deadline = crate::render::next_deadline(*animation_deadline, hdmi);
    }
    presentation.latest = Some(next);
}

fn complete_initial_scene(
    state: &Arc<(Mutex<QueueState>, Condvar)>,
    targets: &HardwareRenderTargets,
    ownership: OledOwnershipState,
    scene: &mut LatestPresentation,
    frame_written: bool,
) {
    if frame_written {
        if let Ok(mut state) = state.0.lock() {
            state.initial_scene_rendered = true;
            state.startup_accepted_oled_frame = Some(scene.shared_oled_frame());
        }
        scene.complete_native(Ok(()));
        return;
    }
    if mark_handoff_failed_decision(ownership) {
        if let Some(handoff) = targets.oled_handoff.as_ref() {
            if let Err(error) = handoff.mark_failed_result() {
                eprintln!(
                    "OLED handoff failure-state publication after initial render failed: {error}"
                );
            }
        }
    }
    scene.complete_native(Err("initial scene OLED render failed".into()));
}
