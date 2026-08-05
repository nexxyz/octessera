use crate::render::{render_snapshot_cached, HardwareRenderCache, HardwareRenderTargets};
use playback_runtime::RuntimeUiPulse;
use serde_json::Value;
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const SHUTDOWN_ACK_TIMEOUT: Duration = Duration::from_millis(750);
const INITIAL_RENDER_ACK_TIMEOUT: Duration = Duration::from_millis(750);

pub struct RenderWorker {
    state: Arc<(Mutex<RenderState>, Condvar)>,
    worker: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
}

enum RenderCommand {
    Snapshot {
        snapshot: Value,
        pulses: Vec<RuntimeUiPulse>,
        rendered_acks: Vec<mpsc::Sender<Result<(), String>>>,
    },
    MarkFirstMenuRendered {
        ack: mpsc::Sender<Result<(), String>>,
    },
    MarkFailed {
        ack: mpsc::Sender<Result<(), String>>,
    },
    Shutdown {
        ack: mpsc::Sender<Result<(), String>>,
    },
    Abort {
        ack: mpsc::Sender<Result<(), String>>,
    },
}

#[derive(Default)]
struct RenderState {
    command: Option<RenderCommand>,
    acknowledged_snapshot_published: bool,
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
        state.command = Some(RenderCommand::MarkFailed { ack: ack_tx });
        ready.notify_one();
        drop(state);
        ack_rx
            .recv_timeout(INITIAL_RENDER_ACK_TIMEOUT)
            .map_err(|error| format!("OLED failure acknowledgement failed: {error}"))?
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
            state.command =
                merge_snapshot_command(state.command.take(), snapshot, pulses, rendered_acks);
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

fn merge_snapshot_command(
    pending: Option<RenderCommand>,
    snapshot: Value,
    mut pulses: Vec<RuntimeUiPulse>,
    mut rendered_acks: Vec<mpsc::Sender<Result<(), String>>>,
) -> Option<RenderCommand> {
    match pending {
        Some(RenderCommand::Shutdown { ack }) => Some(RenderCommand::Shutdown { ack }),
        Some(RenderCommand::Abort { ack }) => Some(RenderCommand::Abort { ack }),
        Some(RenderCommand::Snapshot {
            pulses: mut pending,
            rendered_acks: mut pending_acks,
            ..
        }) => {
            pending.append(&mut pulses);
            pending_acks.append(&mut rendered_acks);
            Some(RenderCommand::Snapshot {
                snapshot,
                pulses: pending,
                rendered_acks: pending_acks,
            })
        }
        Some(RenderCommand::MarkFirstMenuRendered { ack }) => {
            Some(RenderCommand::MarkFirstMenuRendered { ack })
        }
        Some(RenderCommand::MarkFailed { ack }) => Some(RenderCommand::MarkFailed { ack }),
        None => Some(RenderCommand::Snapshot {
            snapshot,
            pulses,
            rendered_acks,
        }),
    }
}

fn render_worker_loop(
    state: Arc<(Mutex<RenderState>, Condvar)>,
    targets: &mut HardwareRenderTargets,
) {
    let mut cache = HardwareRenderCache::default();
    let mut animation_deadline = None;
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
                let rendered_before = cache.oled_render_count();
                animation_deadline = render_snapshot_cached(targets, &snapshot, &mut cache);
                let render_result = if cache.oled_render_count() > rendered_before {
                    Ok(())
                } else {
                    Err("initial snapshot OLED render failed".into())
                };
                if render_result.is_err() {
                    if let Some(handoff) = targets.oled_handoff.as_ref() {
                        handoff.mark_failed();
                    }
                }
                for ack in rendered_acks {
                    let _ = ack.send(render_result.clone());
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
                if let Some(handoff) = targets.oled_handoff.as_ref() {
                    handoff.mark_failed();
                }
                let _ = ack.send(Ok(()));
            }
            Some(RenderCommand::Shutdown { ack }) => {
                crate::render::render_shutdown_splash(&mut targets.oled);
                let _ = targets
                    .seesaw_tx
                    .send(crate::seesaw_io::SeesawCommand::GridFrame([[0; 3]; 64]));
                let _ = targets
                    .seesaw_tx
                    .send(crate::seesaw_io::SeesawCommand::NeoKeyColors([[0; 3]; 4]));
                let display_off = display_off_ack(targets.oled.display_off());
                let _ = ack.send(display_off);
                break;
            }
            Some(RenderCommand::Abort { ack }) => {
                let _ = ack.send(Ok(()));
                break;
            }
            None => {
                animation_deadline =
                    render_sleep_tick_if_uncommanded(&state, targets, &mut cache, Instant::now());
            }
        }
    }
}

fn display_off_ack(result: Result<(), String>) -> Result<(), String> {
    result.map_err(|error| format!("OLED display-off failed: {error}"))
}

fn render_sleep_tick_if_uncommanded(
    state: &Arc<(Mutex<RenderState>, Condvar)>,
    targets: &mut HardwareRenderTargets,
    cache: &mut HardwareRenderCache,
    now: Instant,
) -> Option<Instant> {
    let (lock, _) = &**state;
    let guard = lock.lock().expect("render worker state mutex poisoned");
    if guard.command.is_some() {
        return None;
    }
    drop(guard);
    let sleep_deadline = cache.render_sleep_tick(targets, now);
    let oled_retry_deadline = crate::render::retry_oled_if_due(&mut targets.oled, cache, now);
    crate::render::next_deadline(sleep_deadline, oled_retry_deadline)
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
        if result.timed_out() && guard.command.is_none() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    use octessera_hal::OledSsd1351;

    #[test]
    fn shutdown_ack_preserves_display_off_failure() {
        let (ack_tx, ack_rx) = mpsc::channel();
        ack_tx
            .send(display_off_ack(Err("SPI write failed".into())))
            .unwrap();
        assert_eq!(
            ack_rx.recv().unwrap(),
            Err("OLED display-off failed: SPI write failed".into())
        );
    }

    #[test]
    fn shutdown_ack_timeout_is_bounded() {
        assert_eq!(SHUTDOWN_ACK_TIMEOUT, Duration::from_millis(750));
        assert_eq!(INITIAL_RENDER_ACK_TIMEOUT, Duration::from_millis(750));
    }

    #[test]
    fn pending_wake_command_wins_over_expired_animation_deadline() {
        let state = Arc::new((Mutex::new(RenderState::default()), Condvar::new()));
        {
            let (lock, _) = &*state;
            let mut guard = lock.lock().unwrap();
            guard.command = Some(RenderCommand::Snapshot {
                snapshot: Value::Null,
                pulses: Vec::new(),
                rendered_acks: Vec::new(),
            });
        }

        let command = take_next_command(&state, Some(Instant::now() - Duration::from_millis(1)));
        assert!(matches!(command, Some(RenderCommand::Snapshot { .. })));
    }

    #[test]
    fn snapshot_publication_reports_a_poisoned_worker() {
        let state = Arc::new((Mutex::new(RenderState::default()), Condvar::new()));
        let poison_state = Arc::clone(&state);
        thread::spawn(move || {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _guard = poison_state.0.lock().unwrap();
                panic!("poison render state");
            }));
        })
        .join()
        .unwrap();
        let worker = RenderWorker {
            state,
            worker: Arc::new(Mutex::new(None)),
        };

        assert!(!worker.publish_snapshot(Value::Null, Vec::new()));
    }

    #[test]
    fn snapshot_publication_reports_a_pending_shutdown() {
        let state = Arc::new((Mutex::new(RenderState::default()), Condvar::new()));
        let (ack, _received) = mpsc::channel();
        state.0.lock().unwrap().command = Some(RenderCommand::Shutdown { ack });
        let worker = RenderWorker {
            state,
            worker: Arc::new(Mutex::new(None)),
        };

        assert!(!worker.publish_snapshot(Value::Null, Vec::new()));
    }

    #[test]
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    fn initial_snapshot_ack_is_current_and_cannot_be_reused() {
        let readiness_path = std::env::temp_dir().join(format!(
            "octessera-render-readiness-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut readiness = crate::candidate_readiness::CandidateReadiness::new(
            Some(readiness_path.clone()),
            "render-test".into(),
        );
        let (seesaw_tx, _seesaw_rx) = mpsc::channel();
        let worker = RenderWorker::spawn(HardwareRenderTargets {
            oled: OledSsd1351::new().unwrap(),
            seesaw_tx,
            oled_handoff: None,
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            hdmi: None,
        });
        assert!(worker
            .publish_acknowledged_snapshot(Value::Null, Vec::new())
            .is_ok());
        assert!(!readiness_path.exists());
        readiness.mark_ready().unwrap();
        assert!(readiness_path.is_file());
        assert_eq!(
            worker.publish_acknowledged_snapshot(Value::Null, Vec::new()),
            Err("render worker rejected a second acknowledged snapshot".into())
        );
        worker.abort().unwrap();
        drop(readiness);
        let _ = std::fs::remove_dir_all(readiness_path);
    }
}
