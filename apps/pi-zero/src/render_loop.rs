use crate::render::{
    HardwareRenderCache, HardwareRenderTargets, OledOwnershipStage, OledOwnershipState,
};
#[cfg(test)]
use crate::render_loop_queue::pending_work_wins_over_expired_animation_deadline;
use crate::render_loop_queue::{
    merge_native_scene, NativeSceneCommand, NativeSceneCompletion, RenderCommand, RenderState,
};
use playback_runtime::{
    oled_frame::{OledPresentationMetrics, OledRuntimeErrorMetadata},
    PresentationScene,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

#[cfg(all(
    test,
    not(any(
        feature = "hardware-raspberry-pi-zero-2w",
        feature = "hardware-orange-pi-zero-2w"
    ))
))]
mod hdmi_render_loop_tests;
#[cfg(feature = "profiling")]
mod frame_stats;
mod ownership;
mod presentation;
mod terminal;
#[cfg(test)]
pub(crate) mod test_scenes;
mod worker;
use worker::render_worker_loop;
#[cfg(test)]
use worker::take_next_command;

const SHUTDOWN_ACK_TIMEOUT: Duration = Duration::from_millis(750);
const INITIAL_RENDER_ACK_TIMEOUT: Duration = Duration::from_millis(750);
#[cfg_attr(not(feature = "hardware-orange-pi-zero-2w"), allow(dead_code))]
const OWNERSHIP_ACK_TIMEOUT: Duration = Duration::from_secs(2);
#[derive(Clone)]
pub struct RenderWorker {
    state: Arc<(Mutex<RenderState>, Condvar)>,
    worker: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
}

impl RenderWorker {
    #[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
    pub(crate) fn block_next_native_scene_for_test(
        &self,
    ) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        self.state
            .0
            .lock()
            .expect("render worker state mutex poisoned")
            .native_render_gate = Some((entered_tx, release_rx));
        (entered_rx, release_tx)
    }

    #[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
    pub(crate) fn allow_native_scenes_for_test(&self) {
        self.state
            .0
            .lock()
            .expect("render worker state mutex poisoned")
            .initial_scene_rendered = true;
    }

    pub(crate) fn set_recording_audio(&self, audio: crate::audio::AudioService) {
        if let Ok(mut state) = self.state.0.lock() {
            state.recording_audio = Some(audio);
        }
    }

    #[cfg(test)]
    #[cfg_attr(feature = "hardware-orange-pi-zero-2w", allow(dead_code))]
    pub(crate) fn fail_next_startup_oled_write_for_test(&self) {
        if let Ok(mut state) = self.state.0.lock() {
            state.fail_next_startup_oled_write = true;
        }
    }

    fn take_acknowledged_startup_oled_frame(&self) -> Result<(u64, Arc<[u8]>), String> {
        let mut state =
            self.state.0.lock().map_err(|_| {
                "render worker state mutex poisoned while reading startup OLED frame"
            })?;
        if !state.initial_scene_rendered {
            return Err("initial OLED frame has not been physically acknowledged".into());
        }
        state
            .startup_accepted_oled_frame
            .take()
            .ok_or_else(|| "initial acknowledged OLED frame is unavailable".into())
    }

    pub(crate) fn publish_native_scene(
        &self,
        scene: PresentationScene,
        metrics: OledPresentationMetrics,
        error: Option<OledRuntimeErrorMetadata>,
    ) -> Result<mpsc::Receiver<NativeSceneCompletion>, String> {
        let (completion, receiver) = mpsc::channel();
        let command = NativeSceneCommand {
            scene,
            metrics,
            error,
            completion,
        };
        let (lock, ready) = &*self.state;
        let mut state = lock
            .lock()
            .map_err(|_| "render worker state mutex poisoned during native scene".to_string())?;
        if !state.initial_scene_rendered {
            return Err("initial menu OLED write must be acknowledged before native scenes".into());
        }
        if matches!(
            &state.command,
            Some(
                RenderCommand::Shutdown { .. }
                    | RenderCommand::PreserveTerminal { .. }
                    | RenderCommand::Abort { .. }
            )
        ) {
            return Err("render worker is terminating".into());
        }
        merge_native_scene(&mut state, command);
        ready.notify_one();
        Ok(receiver)
    }

    pub(crate) fn publish_initial_native_scene(
        &self,
        scene: PresentationScene,
        metrics: OledPresentationMetrics,
        error: Option<OledRuntimeErrorMetadata>,
    ) -> Result<(u64, Arc<[u8]>), String> {
        let (completion, receiver) = mpsc::channel();
        let command = NativeSceneCommand {
            scene,
            metrics,
            error,
            completion,
        };
        let (lock, ready) = &*self.state;
        let mut state = lock.lock().map_err(|_| {
            "render worker state mutex poisoned during initial native scene".to_string()
        })?;
        if state.initial_scene_published {
            return Err("render worker rejected a second initial scene".into());
        }
        if state.command.is_some() {
            return Err("render worker is busy during the initial scene".into());
        }
        state.initial_scene_published = true;
        merge_native_scene(&mut state, command);
        ready.notify_one();
        drop(state);
        receiver
            .recv_timeout(INITIAL_RENDER_ACK_TIMEOUT)
            .map_err(|error| format!("initial scene render acknowledgement failed: {error}"))?
            .result?;
        self.take_acknowledged_startup_oled_frame()
    }

    pub fn spawn(mut targets: HardwareRenderTargets) -> Self {
        let state = Arc::new((Mutex::new(RenderState::default()), Condvar::new()));
        let worker_state = Arc::clone(&state);
        let worker = thread::spawn(move || render_worker_loop(worker_state, &mut targets));
        Self {
            state,
            worker: Arc::new(Mutex::new(Some(worker))),
        }
    }

    pub fn mark_first_menu_rendered(&self) -> Result<(), String> {
        let (ack_tx, ack_rx) = mpsc::channel();
        let (lock, ready) = &*self.state;
        let mut state = lock.lock().map_err(|_| {
            "render worker state mutex poisoned during OLED handoff acknowledgement".to_string()
        })?;
        if state.command.is_some() {
            return Err(
                "render worker has a pending command during OLED handoff acknowledgement".into(),
            );
        }
        if !state.initial_scene_rendered {
            return Err("OLED handoff is not acknowledged by a successful native write".into());
        }
        state.command = Some(RenderCommand::MarkFirstMenuRendered { ack: ack_tx });
        ready.notify_one();
        drop(state);
        ack_rx
            .recv_timeout(INITIAL_RENDER_ACK_TIMEOUT)
            .map_err(|error| format!("initial OLED handoff acknowledgement failed: {error}"))?
    }

    pub fn mark_oled_failed(&self) -> Result<(), String> {
        let (ack_tx, ack_rx) = mpsc::channel();
        let (lock, ready) = &*self.state;
        let mut state = lock.lock().map_err(|_| {
            "render worker state mutex poisoned during OLED failure publication".to_string()
        })?;
        if state.command.is_some() {
            return Err(
                "render worker has a pending command during OLED failure publication".into(),
            );
        }
        state.command = Some(RenderCommand::MarkFailed { ack: ack_tx });
        ready.notify_one();
        drop(state);
        let result = ack_rx
            .recv_timeout(INITIAL_RENDER_ACK_TIMEOUT)
            .map_err(|error| format!("OLED failure acknowledgement failed: {error}"))?;
        if let Err(error) = &result {
            eprintln!("OLED failure-state publication acknowledgement failed: {error}");
        }
        result
    }

    #[cfg_attr(not(feature = "hardware-orange-pi-zero-2w"), allow(dead_code))]
    pub(crate) fn ownership_stage(&self, stage: OledOwnershipStage) -> Result<(), String> {
        let (ack_tx, ack_rx) = mpsc::channel();
        let cancellation = Arc::new(AtomicBool::new(false));
        let (lock, ready) = &*self.state;
        let mut state = lock
            .lock()
            .map_err(|_| "render worker state mutex poisoned during OLED ownership".to_string())?;
        if matches!(
            &state.command,
            Some(
                RenderCommand::Shutdown { .. }
                    | RenderCommand::PreserveTerminal { .. }
                    | RenderCommand::Abort { .. },
            )
        ) {
            return Err("render worker is terminating".into());
        }
        if state.command.is_some() {
            return Err("render worker has a pending ownership command".into());
        }
        state.command = Some(RenderCommand::Ownership {
            stage,
            cancellation: Arc::clone(&cancellation),
            ack: ack_tx,
        });
        ready.notify_one();
        drop(state);
        match ack_rx.recv_timeout(OWNERSHIP_ACK_TIMEOUT) {
            Ok(result) => result,
            Err(error) => {
                cancel_ownership(&cancellation);
                Err(format!("OLED ownership acknowledgement failed: {error}"))
            }
        }
    }
}

pub(super) fn take_startup_oled_write_failure(
    state: &Arc<(Mutex<RenderState>, Condvar)>,
    initial_scene: bool,
) -> bool {
    #[cfg(test)]
    {
        initial_scene
            && state
                .0
                .lock()
                .map(|mut state| std::mem::take(&mut state.fail_next_startup_oled_write))
                .unwrap_or(false)
    }
    #[cfg(not(test))]
    {
        let _ = (state, initial_scene);
        false
    }
}
fn display_off_ack(result: Result<(), String>) -> Result<(), String> {
    result.map_err(|error| format!("OLED display-off failed: {error}"))
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
impl crate::orange_oled_suspend_policy::RenderOwnershipControl for RenderWorker {
    fn ownership_stage(&self, stage: OledOwnershipStage) -> Result<(), String> {
        RenderWorker::ownership_stage(self, stage)
    }
}

#[cfg_attr(not(feature = "hardware-orange-pi-zero-2w"), allow(dead_code))]
fn cancel_ownership(cancellation: &AtomicBool) {
    cancellation.store(true, Ordering::Release);
}

fn ownership_command_cancelled(cancellation: &AtomicBool) -> bool {
    cancellation.load(Ordering::Acquire)
}

#[cfg(test)]
mod native_worker_tests;
#[cfg(test)]
mod tests;
