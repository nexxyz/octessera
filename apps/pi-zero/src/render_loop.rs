use crate::oled_frame_cache::OledFramePublication;
use crate::render::{
    HardwareRenderCache, HardwareRenderTargets, OledOwnershipStage, OledOwnershipState,
};
use crate::render_loop_queue::{
    merge_native_scene, merge_snapshot_command, NativeSceneCommand, NativeSceneCompletion,
    RenderCommand, RenderState,
};
#[cfg(test)]
use crate::render_loop_queue::{
    pending_work_wins_over_expired_animation_deadline, SnapshotCommand,
};
use playback_runtime::{
    oled_frame::{OledPresentationMetrics, OledRuntimeErrorMetadata},
    PresentationScene,
};
use serde_json::Value;
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
#[path = "render/hdmi_render_loop_tests.rs"]
mod hdmi_render_loop_tests;
#[path = "render_loop_ownership.rs"]
mod ownership;
#[path = "render_loop_presentation.rs"]
mod presentation;
#[path = "render_loop_terminal.rs"]
mod terminal;
#[path = "render_loop_worker.rs"]
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
            .acknowledged_snapshot_rendered = true;
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

    pub(crate) fn take_acknowledged_startup_oled_frame(&self) -> Result<(u64, Arc<[u8]>), String> {
        let mut state =
            self.state.0.lock().map_err(|_| {
                "render worker state mutex poisoned while reading startup OLED frame"
            })?;
        if !state.acknowledged_snapshot_rendered {
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
        if !state.acknowledged_snapshot_rendered {
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

    pub fn spawn(mut targets: HardwareRenderTargets) -> Self {
        let state = Arc::new((Mutex::new(RenderState::default()), Condvar::new()));
        let worker_state = Arc::clone(&state);
        let worker = thread::spawn(move || render_worker_loop(worker_state, &mut targets));
        Self {
            state,
            worker: Arc::new(Mutex::new(Some(worker))),
        }
    }

    pub fn publish_snapshot(&self, snapshot: Value, oled: OledFramePublication) -> bool {
        if validate_oled_publication(&snapshot, &oled, false).is_err() {
            return false;
        }
        self.publish_snapshot_command(snapshot, oled, Vec::new())
    }

    #[cfg_attr(feature = "hardware-orange-pi-zero-2w", allow(dead_code))]
    pub fn publish_snapshot_with_ack(
        &self,
        snapshot: Value,
        oled: OledFramePublication,
    ) -> Result<(), String> {
        validate_oled_publication(&snapshot, &oled, false)?;
        let (ack_tx, ack_rx) = mpsc::channel();
        if !self.publish_snapshot_command(snapshot, oled, vec![ack_tx]) {
            return Err("render worker rejected snapshot acknowledgement".into());
        }
        ack_rx
            .recv_timeout(INITIAL_RENDER_ACK_TIMEOUT)
            .map_err(|error| format!("snapshot render acknowledgement failed: {error}"))?
    }

    pub fn publish_acknowledged_snapshot(
        &self,
        snapshot: Value,
        oled: OledFramePublication,
    ) -> Result<(), String> {
        validate_oled_publication(&snapshot, &oled, true)?;
        let (ack_tx, ack_rx) = mpsc::channel();
        let (lock, _) = &*self.state;
        let mut state = lock.lock().map_err(|_| {
            "render worker state mutex poisoned during acknowledged snapshot".to_string()
        })?;
        if state.acknowledged_snapshot_published {
            return Err("render worker rejected a second acknowledged snapshot".into());
        }
        state.acknowledged_snapshot_published = true;
        drop(state);
        if !self.publish_snapshot_command(snapshot, oled, vec![ack_tx]) {
            return Err("render worker rejected acknowledged snapshot".into());
        }
        ack_rx
            .recv_timeout(INITIAL_RENDER_ACK_TIMEOUT)
            .map_err(|error| format!("initial snapshot render acknowledgement failed: {error}"))?
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
        if !state.acknowledged_snapshot_rendered {
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

    fn publish_snapshot_command(
        &self,
        snapshot: Value,
        oled: OledFramePublication,
        rendered_acks: Vec<mpsc::Sender<Result<(), String>>>,
    ) -> bool {
        let (lock, ready) = &*self.state;
        if let Ok(mut state) = lock.lock() {
            if matches!(
                &state.command,
                Some(
                    RenderCommand::Shutdown { .. }
                        | RenderCommand::PreserveTerminal { .. }
                        | RenderCommand::Abort { .. },
                )
            ) {
                return false;
            }
            state.snapshot =
                merge_snapshot_command(state.snapshot.take(), snapshot, oled, rendered_acks);
            ready.notify_one();
            true
        } else {
            false
        }
    }
}

pub(super) fn take_startup_oled_write_failure(
    state: &Arc<(Mutex<RenderState>, Condvar)>,
    initial_acknowledged_snapshot: bool,
) -> bool {
    #[cfg(test)]
    {
        initial_acknowledged_snapshot
            && state
                .0
                .lock()
                .map(|mut state| std::mem::take(&mut state.fail_next_startup_oled_write))
                .unwrap_or(false)
    }
    #[cfg(not(test))]
    {
        let _ = (state, initial_acknowledged_snapshot);
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

fn validate_oled_publication(
    snapshot: &Value,
    oled: &OledFramePublication,
    initial: bool,
) -> Result<(), String> {
    let required_revision = snapshot
        .get("oledFrameRevision")
        .and_then(Value::as_u64)
        .filter(|revision| *revision > 0);
    if initial && !oled.is_native() {
        return Err("initial OLED snapshot requires an accepted native frame".into());
    }
    if oled.is_native() && oled.revision() != required_revision {
        return Err("OLED publication does not match snapshot frame revision".into());
    }
    Ok(())
}

pub(super) fn validate_terminal_oled_publication(
    snapshot: &Value,
    oled: &OledFramePublication,
) -> Result<(), String> {
    if !oled.is_native() {
        return Err("terminal OLED snapshot requires an accepted native frame".into());
    }
    let required_revision = snapshot
        .get("oledFrameRevision")
        .and_then(Value::as_u64)
        .filter(|revision| *revision > 0);
    if oled.revision() != required_revision {
        return Err("terminal OLED publication does not match snapshot frame revision".into());
    }
    Ok(())
}

#[cfg_attr(not(feature = "hardware-orange-pi-zero-2w"), allow(dead_code))]
fn cancel_ownership(cancellation: &AtomicBool) {
    cancellation.store(true, Ordering::Release);
}

fn ownership_command_cancelled(cancellation: &AtomicBool) -> bool {
    cancellation.load(Ordering::Acquire)
}

#[cfg(test)]
#[path = "render/native_worker_tests.rs"]
mod native_worker_tests;
#[cfg(test)]
#[path = "render_loop_tests.rs"]
mod tests;
