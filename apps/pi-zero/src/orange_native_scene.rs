use crate::hardware_runtime_scheduler::DisplaySnapshotDue;
use crate::orange_host_adapter::OrangeHostAdapter;
use crate::render_loop::RenderWorker;
use playback_runtime::{NativeRunner, PlaybackRuntime};
use std::sync::mpsc::{self, TryRecvError};
use std::time::Instant;

pub(super) struct OrangeNativeScenePump {
    pending: Vec<(
        u64,
        mpsc::Receiver<crate::render_loop_queue::NativeSceneCompletion>,
    )>,
    generation: Option<u64>,
    last_submission: Instant,
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
            #[cfg(test)]
            capture_count: 0,
        }
    }

    pub(super) fn poll(&mut self, runner: &mut NativeRunner) {
        self.pending
            .retain(|(pending_generation, receiver)| match receiver.try_recv() {
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
                    if self.generation == Some(*pending_generation) {
                        self.generation = None;
                    }
                    false
                }
            });
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
        host.observe_keyboard_capture_mode(scene.hdmi_mode());
        if let Ok(receiver) = worker.publish_native_scene(scene, metrics, error) {
            self.pending.push((generation, receiver));
            self.generation = Some(generation);
        }
        Some(captured_at)
    }
}

#[cfg(test)]
#[path = "orange_native_scene_tests.rs"]
mod tests;
