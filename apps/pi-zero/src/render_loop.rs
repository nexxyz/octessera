use crate::render::{render_snapshot_cached, HardwareRenderCache, HardwareRenderTargets};
use playback_runtime::RuntimeUiPulse;
use serde_json::Value;
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const SHUTDOWN_ACK_TIMEOUT: Duration = Duration::from_millis(750);

pub struct RenderWorker {
    state: Arc<(Mutex<RenderState>, Condvar)>,
    worker: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
}

enum RenderCommand {
    Snapshot {
        snapshot: Value,
        pulses: Vec<RuntimeUiPulse>,
    },
    Shutdown {
        ack: mpsc::Sender<Result<(), String>>,
    },
}

#[derive(Default)]
struct RenderState {
    command: Option<RenderCommand>,
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
        let (lock, ready) = &*self.state;
        if let Ok(mut state) = lock.lock() {
            if matches!(&state.command, Some(RenderCommand::Shutdown { .. })) {
                return false;
            }
            state.command = merge_snapshot_command(state.command.take(), snapshot, pulses);
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
}

fn merge_snapshot_command(
    pending: Option<RenderCommand>,
    snapshot: Value,
    mut pulses: Vec<RuntimeUiPulse>,
) -> Option<RenderCommand> {
    match pending {
        Some(RenderCommand::Shutdown { ack }) => Some(RenderCommand::Shutdown { ack }),
        Some(RenderCommand::Snapshot {
            pulses: mut pending,
            ..
        }) => {
            pending.append(&mut pulses);
            Some(RenderCommand::Snapshot {
                snapshot,
                pulses: pending,
            })
        }
        None => Some(RenderCommand::Snapshot { snapshot, pulses }),
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
            Some(RenderCommand::Snapshot { snapshot, pulses }) => {
                for pulse in pulses {
                    cache.apply_ui_pulse(pulse);
                }
                let snapshot = cache.snapshot_with_transients(&snapshot);
                animation_deadline = render_snapshot_cached(targets, &snapshot, &mut cache);
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
    cache.render_sleep_tick(targets, now)
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
}
