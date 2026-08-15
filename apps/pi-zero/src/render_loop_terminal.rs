use super::{
    display_off_ack, HardwareRenderCache, HardwareRenderTargets, OledFramePublication,
    OledOwnershipState, RenderWorker, SHUTDOWN_ACK_TIMEOUT,
};
use crate::render::{force_latest_oled, render_shutdown_splash, restore_for_render};
use crate::render_loop_queue::{reject_pending_command, RenderCommand};
use crate::seesaw_io::SeesawCommand;
use serde_json::Value;
use std::sync::mpsc;
#[cfg(test)]
use std::sync::{Arc, Condvar, Mutex};

impl RenderWorker {
    pub(crate) fn publish_shutdown(&self) -> Result<(), String> {
        let (ack_tx, ack_rx) = mpsc::channel();
        let (lock, ready) = &*self.state;
        if let Ok(mut state) = lock.lock() {
            reject_pending_command(&mut state, "render worker was preempted by shutdown");
            state.command = Some(RenderCommand::Shutdown { ack: ack_tx });
            ready.notify_one();
        } else {
            return Err("render worker state mutex poisoned during shutdown".into());
        }
        let ack_result = ack_rx
            .recv_timeout(SHUTDOWN_ACK_TIMEOUT)
            .map_err(|error| format!("render shutdown acknowledgement failed: {error}"))?;
        let worker = self
            .worker
            .lock()
            .ok()
            .and_then(|mut worker| worker.take())
            .ok_or_else(|| "render worker handle unavailable during shutdown".to_string())?;
        worker
            .join()
            .map_err(|_| "render worker panicked during shutdown".to_string())?;
        ack_result
    }

    pub(crate) fn publish_terminal_preserving(
        &self,
        snapshot: Value,
        oled: OledFramePublication,
    ) -> Result<(), String> {
        super::validate_terminal_oled_publication(&snapshot, &oled)?;
        let (ack_tx, ack_rx) = mpsc::channel();
        let (lock, ready) = &*self.state;
        if let Ok(mut state) = lock.lock() {
            reject_pending_command(
                &mut state,
                "render worker was preempted by preserving terminal teardown",
            );
            state.command = Some(RenderCommand::PreserveTerminal {
                snapshot,
                oled,
                ack: ack_tx,
            });
            ready.notify_one();
        } else {
            return Err("render worker state mutex poisoned during terminal teardown".into());
        }
        let ack_result = ack_rx
            .recv_timeout(SHUTDOWN_ACK_TIMEOUT)
            .map_err(|error| format!("render terminal teardown acknowledgement failed: {error}"));
        let join_result = self.join_worker("terminal teardown");
        match (ack_result, join_result) {
            (Ok(ack), Ok(())) => ack,
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) | (Err(_), Err(error)) => Err(error),
        }
    }

    #[cfg_attr(not(feature = "hardware-orange-pi-zero-2w"), allow(dead_code))]
    pub(crate) fn is_terminated(&self) -> bool {
        self.worker.lock().map_or(true, |worker| worker.is_none())
    }

    #[cfg(test)]
    #[cfg_attr(not(feature = "hardware-orange-pi-zero-2w"), allow(dead_code))]
    pub(crate) fn terminated_for_test() -> Self {
        Self {
            state: Arc::new((Mutex::new(Default::default()), Condvar::new())),
            worker: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn abort(&self) -> Result<(), String> {
        let (ack_tx, ack_rx) = mpsc::channel();
        let (lock, ready) = &*self.state;
        if let Ok(mut state) = lock.lock() {
            reject_pending_command(&mut state, "render worker was preempted by abort");
            state.command = Some(RenderCommand::Abort { ack: ack_tx });
            ready.notify_one();
        } else {
            return Err("render worker state mutex poisoned during abort".into());
        }
        let result = ack_rx
            .recv_timeout(SHUTDOWN_ACK_TIMEOUT)
            .map_err(|error| format!("render abort acknowledgement failed: {error}"))?;
        let worker = self
            .worker
            .lock()
            .ok()
            .and_then(|mut worker| worker.take())
            .ok_or_else(|| "render worker handle unavailable during abort".to_string())?;
        worker
            .join()
            .map_err(|_| "render worker panicked during abort".to_string())?;
        result
    }

    fn join_worker(&self, operation: &str) -> Result<(), String> {
        let worker = self
            .worker
            .lock()
            .ok()
            .and_then(|mut worker| worker.take())
            .ok_or_else(|| format!("render worker handle unavailable during {operation}"))?;
        worker
            .join()
            .map_err(|_| format!("render worker panicked during {operation}"))
    }
}

pub(super) fn handle_shutdown(
    targets: &mut HardwareRenderTargets,
    cache: &mut HardwareRenderCache,
    latest_snapshot: &Option<Value>,
    latest_oled: &Option<OledFramePublication>,
    ownership: &mut OledOwnershipState,
) -> Result<(), String> {
    let restore_result =
        restore_for_render(targets, cache, latest_snapshot, latest_oled, ownership);
    if restore_result.is_ok() {
        render_shutdown_splash(&mut targets.oled);
    }
    let _ = targets
        .seesaw_tx
        .send(SeesawCommand::GridFrame([[0; 3]; 64]));
    let _ = targets
        .seesaw_tx
        .send(SeesawCommand::NeoKeyColors([[0; 3]; 4]));
    display_off_ack(restore_result.and(targets.oled.display_off()))
}

pub(super) fn handle_preserve_terminal(
    targets: &mut HardwareRenderTargets,
    cache: &mut HardwareRenderCache,
    latest_snapshot: &Option<Value>,
    latest_oled: &Option<OledFramePublication>,
    ownership: &mut OledOwnershipState,
    snapshot: &Value,
    oled: &OledFramePublication,
) -> Result<(), String> {
    let mut errors = Vec::new();
    if let Err(error) = restore_for_render(targets, cache, latest_snapshot, latest_oled, ownership)
    {
        errors.push(format!("OLED ownership restore failed: {error}"));
    }
    if let Err(error) = force_latest_oled(targets, snapshot, oled, cache) {
        errors.push(format!("terminal OLED frame write failed: {error}"));
    }
    if let Err(error) = targets
        .seesaw_tx
        .send(SeesawCommand::GridFrame([[0; 3]; 64]))
    {
        errors.push(format!("grid LED zeroing failed: {error}"));
    }
    if let Err(error) = targets
        .seesaw_tx
        .send(SeesawCommand::NeoKeyColors([[0; 3]; 4]))
    {
        errors.push(format!("NeoKey LED zeroing failed: {error}"));
    }
    if let Err(error) = targets.oled.detach_preserving() {
        errors.push(format!("OLED preserving detach failed: {error}"));
    }
    if let Some(handoff) = targets.oled_handoff.as_mut() {
        if let Err(error) = handoff.detach_preserving() {
            errors.push(format!("OLED handoff preserving detach failed: {error}"));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

pub(super) fn handle_abort(
    targets: &mut HardwareRenderTargets,
    cache: &mut HardwareRenderCache,
    latest_snapshot: &Option<Value>,
    latest_oled: &Option<OledFramePublication>,
    ownership: &mut OledOwnershipState,
) -> Result<(), String> {
    restore_for_render(targets, cache, latest_snapshot, latest_oled, ownership)
}
