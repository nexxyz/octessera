use crate::render::{
    initial_snapshot_render_result, mark_handoff_failed_decision, ownership_stage_for_render,
    render_leds_only, render_snapshot_cached, restore_after_dropped_ack_for_render,
    restore_for_render, retry_oled_decision, select_snapshot_render, snapshot_requires_oled_ack,
    HardwareRenderCache, HardwareRenderTargets, OledOwnershipStage, OledOwnershipState,
    SnapshotRenderDecision,
};
use crate::render_loop_queue::{
    merge_snapshot_command, pending_work_wins_over_expired_animation_deadline,
    reject_pending_command, RenderCommand, RenderState, SnapshotCommand,
};
use playback_runtime::RuntimeUiPulse;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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
    pub fn spawn(mut targets: HardwareRenderTargets) -> Self {
        let state = Arc::new((Mutex::new(RenderState::default()), Condvar::new()));
        let worker_state = Arc::clone(&state);
        let worker = thread::spawn(move || render_worker_loop(worker_state, &mut targets));
        Self {
            state,
            worker: Arc::new(Mutex::new(Some(worker))),
        }
    }

    pub fn publish_snapshot(&self, snapshot: Value, pulses: Vec<RuntimeUiPulse>) -> bool {
        self.publish_snapshot_command(snapshot, pulses, Vec::new())
    }

    pub fn publish_acknowledged_snapshot(
        &self,
        snapshot: Value,
        pulses: Vec<RuntimeUiPulse>,
    ) -> Result<(), String> {
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
        if !self.publish_snapshot_command(snapshot, pulses, vec![ack_tx]) {
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
        ack_rx
            .recv_timeout(INITIAL_RENDER_ACK_TIMEOUT)
            .map_err(|error| format!("OLED failure acknowledgement failed: {error}"))?
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
            Some(RenderCommand::Shutdown { .. } | RenderCommand::Abort { .. })
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
        pulses: Vec<RuntimeUiPulse>,
        rendered_acks: Vec<mpsc::Sender<Result<(), String>>>,
    ) -> bool {
        let (lock, ready) = &*self.state;
        if let Ok(mut state) = lock.lock() {
            if matches!(
                &state.command,
                Some(RenderCommand::Shutdown { .. } | RenderCommand::Abort { .. })
            ) {
                return false;
            }
            state.snapshot =
                merge_snapshot_command(state.snapshot.take(), snapshot, pulses, rendered_acks);
            ready.notify_one();
            true
        } else {
            false
        }
    }

    pub fn publish_shutdown(&self) -> Result<(), String> {
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

    pub fn abort(&self) -> Result<(), String> {
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
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
impl crate::orange_oled_suspend_policy::RenderOwnershipControl for RenderWorker {
    fn ownership_stage(&self, stage: OledOwnershipStage) -> Result<(), String> {
        RenderWorker::ownership_stage(self, stage)
    }
}

fn render_worker_loop(
    state: Arc<(Mutex<RenderState>, Condvar)>,
    targets: &mut HardwareRenderTargets,
) {
    let mut cache = HardwareRenderCache::default();
    let mut animation_deadline = None;
    let mut latest_snapshot = None;
    let mut ownership = OledOwnershipState::default();
    loop {
        let command = take_next_command(&state, animation_deadline);
        match command {
            Some(RenderCommand::Snapshot {
                snapshot,
                pulses,
                rendered_acks,
            }) => {
                for pulse in pulses {
                    cache.apply_ui_pulse(pulse);
                }
                let snapshot = cache.snapshot_with_transients(&snapshot);
                latest_snapshot = Some(snapshot.clone());
                let require_oled_ack = snapshot_requires_oled_ack(rendered_acks.len());
                let mut full_render_result = None;
                animation_deadline = select_snapshot_render(ownership, |decision| match decision {
                    SnapshotRenderDecision::OledAndLeds => {
                        let rendered_before = require_oled_ack.then(|| cache.oled_render_count());
                        let deadline = render_snapshot_cached(targets, &snapshot, &mut cache);
                        let render_result = initial_snapshot_render_result(
                            require_oled_ack,
                            rendered_before
                                .is_some_and(|before| cache.oled_render_count() > before),
                        );
                        full_render_result = render_result;
                        deadline
                    }
                    SnapshotRenderDecision::LedsOnly => {
                        let deadline =
                            render_leds_only(targets, &snapshot, &mut cache, Instant::now());
                        full_render_result =
                            initial_snapshot_render_result(require_oled_ack, false);
                        deadline
                    }
                });
                if full_render_result
                    .as_ref()
                    .is_some_and(|result| result.is_err())
                    && mark_handoff_failed_decision(ownership)
                {
                    if let Some(handoff) = targets.oled_handoff.as_ref() {
                        handoff.mark_failed();
                    }
                }
                if let Some(render_result) = full_render_result {
                    for ack in rendered_acks {
                        let _ = ack.send(render_result.clone());
                    }
                } else {
                    for ack in rendered_acks {
                        let _ = ack.send(Ok(()));
                    }
                }
            }
            Some(RenderCommand::MarkFirstMenuRendered { ack }) => {
                let result = targets
                    .oled_handoff
                    .as_mut()
                    .map_or(Ok(()), |handoff| handoff.mark_first_menu_rendered());
                let _ = ack.send(result);
            }
            Some(RenderCommand::MarkFailed { ack }) => {
                if mark_handoff_failed_decision(ownership) {
                    if let Some(handoff) = targets.oled_handoff.as_ref() {
                        handoff.mark_failed();
                    }
                }
                let _ = ack.send(Ok(()));
            }
            Some(RenderCommand::Ownership {
                stage,
                cancellation,
                ack,
            }) => {
                let cancelled = ownership_command_cancelled(&cancellation);
                if !cancelled && stage == OledOwnershipStage::ResumeComplete {
                    if let Some(SnapshotCommand {
                        snapshot,
                        pulses,
                        rendered_acks,
                    }) = state
                        .0
                        .lock()
                        .ok()
                        .and_then(|mut state| state.snapshot.take())
                    {
                        for pulse in pulses {
                            cache.apply_ui_pulse(pulse);
                        }
                        let snapshot = cache.snapshot_with_transients(&snapshot);
                        latest_snapshot = Some(snapshot.clone());
                        animation_deadline =
                            render_leds_only(targets, &snapshot, &mut cache, Instant::now());
                        for ack in rendered_acks {
                            let _ = ack.send(Ok(()));
                        }
                    }
                }
                let result = if cancelled {
                    Err("OLED ownership command was cancelled".into())
                } else {
                    ownership_stage_for_render(
                        stage,
                        targets,
                        &mut cache,
                        &latest_snapshot,
                        &mut ownership,
                    )
                };
                if let Err(error) = restore_after_dropped_ack_for_render(
                    ack.send(result).is_err(),
                    targets,
                    &mut cache,
                    &latest_snapshot,
                    &mut ownership,
                ) {
                    eprintln!(
                        "OLED ownership rollback after dropped acknowledgement failed: {error}"
                    );
                }
            }
            Some(RenderCommand::Shutdown { ack }) => {
                let restore_result =
                    restore_for_render(targets, &mut cache, &latest_snapshot, &mut ownership);
                if restore_result.is_ok() {
                    crate::render::render_shutdown_splash(&mut targets.oled);
                }
                let _ = targets
                    .seesaw_tx
                    .send(crate::seesaw_io::SeesawCommand::GridFrame([[0; 3]; 64]));
                let _ = targets
                    .seesaw_tx
                    .send(crate::seesaw_io::SeesawCommand::NeoKeyColors([[0; 3]; 4]));
                let display_off = display_off_ack(restore_result.and(targets.oled.display_off()));
                let _ = ack.send(display_off);
                break;
            }
            Some(RenderCommand::Abort { ack }) => {
                let result =
                    restore_for_render(targets, &mut cache, &latest_snapshot, &mut ownership);
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
                    let sleep_deadline = cache.render_sleep_tick(targets, now);
                    let retry_deadline = if retry_oled_decision(ownership) {
                        crate::render::retry_oled_if_due(&mut targets.oled, &mut cache, now)
                    } else {
                        None
                    };
                    animation_deadline =
                        crate::render::next_deadline(sleep_deadline, retry_deadline);
                }
            }
        }
    }
}

fn display_off_ack(result: Result<(), String>) -> Result<(), String> {
    result.map_err(|error| format!("OLED display-off failed: {error}"))
}

#[cfg_attr(not(feature = "hardware-orange-pi-zero-2w"), allow(dead_code))]
fn cancel_ownership(cancellation: &AtomicBool) {
    cancellation.store(true, Ordering::Release);
}

fn ownership_command_cancelled(cancellation: &AtomicBool) -> bool {
    cancellation.load(Ordering::Acquire)
}

fn take_next_command(
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
            return Some(RenderCommand::Snapshot {
                snapshot: snapshot.snapshot,
                pulses: snapshot.pulses,
                rendered_acks: snapshot.rendered_acks,
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
#[cfg(test)]
#[path = "render_loop_tests.rs"]
mod tests;
