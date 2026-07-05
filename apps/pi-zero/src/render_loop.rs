use crate::render::{render_snapshot_cached, HardwareRenderCache, HardwareRenderTargets};
use playback_runtime::RuntimeUiPulse;
use serde_json::Value;
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

const SHUTDOWN_RENDER_TIMEOUT: Duration = Duration::from_millis(750);

pub struct RenderWorker {
    state: Arc<(Mutex<RenderState>, Condvar)>,
}

enum RenderCommand {
    Snapshot {
        snapshot: Value,
        pulses: Vec<RuntimeUiPulse>,
    },
    Shutdown {
        ack: mpsc::Sender<()>,
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
        thread::spawn(move || render_worker_loop(worker_state, &mut targets));
        Self { state }
    }

    pub fn publish_snapshot(&self, snapshot: Value, pulses: Vec<RuntimeUiPulse>) {
        let (lock, ready) = &*self.state;
        if let Ok(mut state) = lock.lock() {
            state.command = merge_snapshot_command(state.command.take(), snapshot, pulses);
            ready.notify_one();
        }
    }

    pub fn publish_shutdown(&self) -> bool {
        let (ack_tx, ack_rx) = mpsc::channel();
        let (lock, ready) = &*self.state;
        if let Ok(mut state) = lock.lock() {
            state.command = Some(RenderCommand::Shutdown { ack: ack_tx });
            ready.notify_one();
        } else {
            return false;
        }
        ack_rx.recv_timeout(SHUTDOWN_RENDER_TIMEOUT).is_ok()
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
    loop {
        let command = take_next_command(&state);
        match command {
            RenderCommand::Snapshot { snapshot, pulses } => {
                for pulse in pulses {
                    cache.apply_ui_pulse(pulse);
                }
                let snapshot = cache.snapshot_with_transients(&snapshot);
                render_snapshot_cached(targets, &snapshot, &mut cache);
            }
            RenderCommand::Shutdown { ack } => {
                crate::render::render_shutdown_splash(&mut targets.oled);
                let _ = targets
                    .seesaw_tx
                    .send(crate::seesaw_io::SeesawCommand::GridFrame([[0; 3]; 64]));
                let _ = targets
                    .seesaw_tx
                    .send(crate::seesaw_io::SeesawCommand::NeoKeyColors([[0; 3]; 4]));
                let _ = ack.send(());
                break;
            }
        }
    }
}

fn take_next_command(state: &Arc<(Mutex<RenderState>, Condvar)>) -> RenderCommand {
    let (lock, ready) = &**state;
    let mut guard = lock.lock().expect("render worker state mutex poisoned");
    loop {
        if let Some(command) = guard.command.take() {
            return command;
        }
        guard = ready
            .wait(guard)
            .expect("render worker state mutex poisoned while waiting");
    }
}
