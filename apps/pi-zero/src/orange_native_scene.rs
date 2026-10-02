use crate::hardware_runtime_scheduler::DisplaySnapshotDue;
use crate::orange_host_adapter::OrangeHostAdapter;
use crate::render_loop::RenderWorker;
use playback_runtime::{NativeRunner, PlaybackRuntime};
use std::sync::mpsc::{self, TryRecvError};
use std::time::Instant;

struct PendingNativeScene {
    generation: u64,
    cutoff_value: Option<u16>,
    receiver: mpsc::Receiver<crate::render_loop_queue::NativeSceneCompletion>,
}

pub(super) struct OrangeNativeScenePump {
    pending: Vec<PendingNativeScene>,
    generation: Option<u64>,
    last_submission: Instant,
    profile_capture: bool,
    capture_duration: Option<std::time::Duration>,
    timing_cutoff_targets: Option<[u16; 2]>,
    timing_cutoff_acceptances: [Option<(u64, u16)>; 2],
    #[cfg(test)]
    capture_count: usize,
}

impl OrangeNativeScenePump {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            pending: Vec::new(),
            generation: None,
            last_submission: now
                .checked_sub(crate::hardware_runtime_scheduler::SNAPSHOT_TICK)
                .unwrap_or(now),
            profile_capture: false,
            capture_duration: None,
            timing_cutoff_targets: None,
            timing_cutoff_acceptances: [None, None],
            #[cfg(test)]
            capture_count: 0,
        }
    }

    pub(super) fn poll(&mut self, runner: &mut NativeRunner) {
        let mut completed = Vec::new();
        self.pending
            .retain(|pending| match pending.receiver.try_recv() {
                Ok(receipt) => {
                    completed.push((pending.generation, pending.cutoff_value, receipt));
                    false
                }
                Err(TryRecvError::Empty) => true,
                Err(TryRecvError::Disconnected) => {
                    if self.generation == Some(pending.generation) {
                        self.generation = None;
                    }
                    false
                }
            });
        for (pending_generation, cutoff_value, receipt) in completed {
            if receipt.result.is_ok() {
                runner.acknowledge_display_scene(receipt.generation);
                debug_assert!(receipt.frame_revision.is_some());
                self.record_completed_cutoff_value(cutoff_value, &receipt);
            }
            if self.generation == Some(receipt.generation)
                || (receipt.result.is_err() && self.generation == Some(pending_generation))
            {
                self.generation = None;
            }
        }
    }

    pub(super) fn submit(
        &mut self,
        now: Instant,
        due: DisplaySnapshotDue,
        playback: &PlaybackRuntime,
        runner: &mut NativeRunner,
        host: &mut OrangeHostAdapter,
        worker: &RenderWorker,
    ) -> Option<Instant> {
        self.capture_duration = None;
        if playback
            .last_snapshot()
            .and_then(|snapshot| snapshot.get("runtimeError"))
            .is_some_and(|error| !error.is_null())
        {
            return None;
        }
        let pending_generation = runner.pending_display_scene_generation();
        if self.generation.is_some_and(|generation| {
            pending_generation.is_none() || pending_generation == Some(generation)
        }) {
            return None;
        }
        if pending_generation.is_none() && !due.any() {
            return None;
        }
        if now.duration_since(self.last_submission)
            < crate::hardware_runtime_scheduler::SNAPSHOT_TICK
        {
            return None;
        }
        #[cfg(test)]
        {
            self.capture_count += 1;
        }
        let capture_started = self.profile_capture.then(Instant::now);
        let capture = runner.capture_display_scene();
        if let Some(started) = capture_started {
            self.capture_duration = Some(started.elapsed());
        }
        let Ok(scene) = capture else {
            return None;
        };
        let generation = scene.generation();
        if self.generation == Some(generation) {
            return None;
        }
        let (metrics, error) = playback.native_presentation_state();
        let cutoff_value = self
            .timing_cutoff_targets
            .and_then(|_| selected_cutoff_display_value(&scene, metrics.clone(), error.clone()));
        let captured_at = Instant::now();
        self.last_submission = captured_at;
        host.core.observe_keyboard_capture_mode(scene.hdmi_mode());
        if let Ok(receiver) = worker.publish_native_scene(scene, metrics, error) {
            self.pending.push(PendingNativeScene {
                generation,
                cutoff_value,
                receiver,
            });
            self.generation = Some(generation);
        }
        Some(captured_at)
    }

    pub(super) fn set_capture_profile_enabled(&mut self, enabled: bool) {
        self.profile_capture = enabled;
    }

    pub(super) fn take_capture_duration(&mut self) -> Option<std::time::Duration> {
        self.capture_duration.take()
    }

    pub(super) fn set_timing_cutoff_targets(&mut self, targets: [u16; 2]) {
        self.timing_cutoff_targets = (targets[0] != targets[1]).then_some(targets);
        self.timing_cutoff_acceptances = [None, None];
    }

    pub(super) fn timing_cutoff_acceptances(&self) -> Option<[(u64, u16); 2]> {
        Some([
            self.timing_cutoff_acceptances[0]?,
            self.timing_cutoff_acceptances[1]?,
        ])
    }

    fn record_timing_cutoff_acceptance(&mut self, revision: u64, value: u16) {
        let Some(index) = self
            .timing_cutoff_targets
            .and_then(|targets| targets.iter().position(|target| *target == value))
        else {
            return;
        };
        self.timing_cutoff_acceptances[index].get_or_insert((revision, value));
    }

    pub(super) fn record_completed_cutoff_value(
        &mut self,
        cutoff_value: Option<u16>,
        receipt: &crate::render_loop_queue::NativeSceneCompletion,
    ) {
        if receipt.result.is_ok() {
            if let (Some(value), Some(revision)) = (cutoff_value, receipt.frame_revision) {
                self.record_timing_cutoff_acceptance(revision, value);
            }
        }
    }
}

pub(super) fn selected_cutoff_display_value(
    scene: &playback_runtime::PresentationScene,
    metrics: playback_runtime::oled_frame::OledPresentationMetrics,
    error: Option<playback_runtime::oled_frame::OledRuntimeErrorMetadata>,
) -> Option<u16> {
    let presentation = scene.oled_presentation_input(metrics, error);
    visible_cutoff_display_value(&presentation)
}

pub(super) fn visible_cutoff_display_value(
    presentation: &playback_runtime::oled_frame::OledPresentationInput,
) -> Option<u16> {
    if presentation.display.off
        || presentation.display.splash != playback_runtime::oled_frame::OledSplash::None
        || presentation.display.editing
        || !presentation.display.title.contains("Filter")
        || presentation.runtime_error.is_some()
    {
        return None;
    }
    let row = presentation.selected_row?;
    let line = presentation.display.lines.get(row)?;
    if !line.contains("Cutoff") {
        return None;
    }
    line.split_whitespace()
        .last()?
        .parse::<u16>()
        .ok()
        .filter(|value| *value <= 255)
}

#[cfg(test)]
mod timing_evidence_tests {
    use super::*;
    use playback_runtime::oled_frame::{OledDisplayInput, OledPresentationInput, OledSplash};

    #[test]
    fn cutoff_rows_hidden_by_overlays_or_wrong_focus_are_not_evidence() {
        let mut presentation = OledPresentationInput {
            display: OledDisplayInput {
                title: "/Shape/Instruments/I1: synth/Synth/Filter".into(),
                lines: vec!["> 1-Cutoff 140".into()],
                ..OledDisplayInput::default()
            },
            selected_row: Some(0),
            ..OledPresentationInput::default()
        };
        assert_eq!(visible_cutoff_display_value(&presentation), Some(140));

        presentation.runtime_error = Some(Default::default());
        assert_eq!(visible_cutoff_display_value(&presentation), None);
        presentation.runtime_error = None;
        presentation.display.splash = OledSplash::Sleep;
        assert_eq!(visible_cutoff_display_value(&presentation), None);
        presentation.display.splash = OledSplash::None;
        presentation.display.off = true;
        assert_eq!(visible_cutoff_display_value(&presentation), None);
        presentation.display.off = false;
        presentation.display.title = "/Shape/Instruments/I1: synth/Synth/Osc 1".into();
        assert_eq!(visible_cutoff_display_value(&presentation), None);
        presentation.display.title = "/Shape/Instruments/I1: synth/Synth/Filter".into();
        presentation.display.editing = true;
        assert_eq!(visible_cutoff_display_value(&presentation), None);
        presentation.display.editing = false;
        presentation.selected_row = None;
        assert_eq!(visible_cutoff_display_value(&presentation), None);
    }

    #[test]
    fn cutoff_revision_evidence_requires_successful_matching_native_frames() {
        let mut pump = OrangeNativeScenePump::new(Instant::now());
        pump.set_timing_cutoff_targets([140, 141]);
        let failed = crate::render_loop_queue::NativeSceneCompletion {
            generation: 1,
            result: Err("OLED write failed".into()),
            frame_revision: None,
        };
        let unrelated = crate::render_loop_queue::NativeSceneCompletion {
            generation: 2,
            result: Ok(()),
            frame_revision: Some(20),
        };
        pump.record_completed_cutoff_value(Some(140), &failed);
        pump.record_completed_cutoff_value(Some(139), &unrelated);
        assert!(pump.timing_cutoff_acceptances().is_none());

        let first = crate::render_loop_queue::NativeSceneCompletion {
            generation: 3,
            result: Ok(()),
            frame_revision: Some(21),
        };
        let second = crate::render_loop_queue::NativeSceneCompletion {
            generation: 4,
            result: Ok(()),
            frame_revision: Some(22),
        };
        pump.record_completed_cutoff_value(Some(140), &first);
        pump.record_completed_cutoff_value(Some(141), &second);
        pump.record_completed_cutoff_value(Some(140), &unrelated);
        assert_eq!(
            pump.timing_cutoff_acceptances(),
            Some([(21, 140), (22, 141)])
        );
    }
}

#[cfg(test)]
#[path = "orange_native_scene_tests.rs"]
mod tests;
