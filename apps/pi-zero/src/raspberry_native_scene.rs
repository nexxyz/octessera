use crate::hardware_runtime_scheduler::DisplaySnapshotDue;
use crate::host_adapter::PiPlaybackHostAdapter;
use crate::render_loop::RenderWorker;
use playback_runtime::{NativeRunner, PlaybackRuntime};
use std::sync::mpsc::{self, TryRecvError};
use std::time::Instant;

pub(crate) struct NativeScenePump {
    pending: Vec<PendingNativeScene>,
    generation: Option<u64>,
    last_submission: Instant,
    timing_cutoff_targets: Option<[u16; 2]>,
    timing_cutoff_acceptances: [Option<(u64, u16)>; 2],
    #[cfg(test)]
    capture_count: usize,
}

struct PendingNativeScene {
    generation: u64,
    cutoff_value: Option<u16>,
    receiver: mpsc::Receiver<crate::render_loop_queue::NativeSceneCompletion>,
}

impl NativeScenePump {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            pending: Vec::new(),
            generation: None,
            last_submission: now
                .checked_sub(crate::hardware_runtime_scheduler::SNAPSHOT_TICK)
                .unwrap_or(now),
            timing_cutoff_targets: None,
            timing_cutoff_acceptances: [None, None],
            #[cfg(test)]
            capture_count: 0,
        }
    }

    pub(crate) fn poll(&mut self, runner: &mut NativeRunner) {
        if self.timing_cutoff_targets.is_some() {
            self.poll_timing_cutoff(runner);
        } else {
            self.poll_regular(runner);
        }
    }

    fn poll_regular(&mut self, runner: &mut NativeRunner) {
        self.pending
            .retain(|pending| match pending.receiver.try_recv() {
                Ok(receipt) => {
                    if receipt.result.is_ok() {
                        runner.acknowledge_display_scene(receipt.generation);
                        debug_assert!(receipt.frame_revision.is_some());
                        if self.generation == Some(receipt.generation) {
                            self.generation = None;
                        }
                    } else if self.generation == Some(receipt.generation) {
                        self.generation = None;
                    }
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
    }

    fn poll_timing_cutoff(&mut self, runner: &mut NativeRunner) {
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
                if receipt.generation == pending_generation {
                    if let (Some(value), Some(revision)) = (cutoff_value, receipt.frame_revision) {
                        self.record_timing_cutoff_acceptance(revision, value);
                    }
                }
            }
            if self.generation == Some(receipt.generation) {
                self.generation = None;
            }
        }
    }

    pub(crate) fn submit(
        &mut self,
        now: Instant,
        due: DisplaySnapshotDue,
        playback: &PlaybackRuntime,
        runner: &mut NativeRunner,
        adapter: &mut PiPlaybackHostAdapter,
        worker: &RenderWorker,
    ) -> Option<Instant> {
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
        let Ok(scene) = runner.capture_display_scene() else {
            return None;
        };
        let generation = scene.generation();
        if self.generation == Some(generation) {
            return None;
        }
        let captured_at = Instant::now();
        self.last_submission = captured_at;
        let (metrics, error) = playback.native_presentation_state();
        let cutoff_value = self
            .timing_cutoff_targets
            .and_then(|_| selected_cutoff_display_value(&scene, metrics.clone(), error.clone()));
        adapter
            .core
            .observe_keyboard_capture_mode(scene.hdmi_mode());
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

    pub(crate) fn set_timing_cutoff_targets(&mut self, targets: [u16; 2]) {
        self.timing_cutoff_targets = (targets[0] != targets[1]).then_some(targets);
        self.timing_cutoff_acceptances = [None, None];
    }

    pub(crate) fn timing_cutoff_acceptances(&self) -> Option<[(u64, u16); 2]> {
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
}

fn selected_cutoff_display_value(
    scene: &playback_runtime::PresentationScene,
    metrics: playback_runtime::oled_frame::OledPresentationMetrics,
    error: Option<playback_runtime::oled_frame::OledRuntimeErrorMetadata>,
) -> Option<u16> {
    let presentation = scene.oled_presentation_input(metrics, error);
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
#[path = "raspberry_native_scene_tests.rs"]
mod tests;
