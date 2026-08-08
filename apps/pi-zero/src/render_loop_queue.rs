use crate::render::OledOwnershipStage;
use playback_runtime::RuntimeUiPulse;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;

pub(crate) enum RenderCommand {
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
    #[cfg_attr(not(feature = "hardware-orange-pi-zero-2w"), allow(dead_code))]
    Ownership {
        stage: OledOwnershipStage,
        cancellation: Arc<AtomicBool>,
        ack: mpsc::Sender<Result<(), String>>,
    },
}

pub(crate) struct SnapshotCommand {
    pub(crate) snapshot: Value,
    pub(crate) pulses: Vec<RuntimeUiPulse>,
    pub(crate) rendered_acks: Vec<mpsc::Sender<Result<(), String>>>,
}

#[derive(Default)]
pub(crate) struct RenderState {
    pub(crate) command: Option<RenderCommand>,
    pub(crate) snapshot: Option<SnapshotCommand>,
    pub(crate) acknowledged_snapshot_published: bool,
}

pub(crate) fn pending_work_wins_over_expired_animation_deadline(state: &RenderState) -> bool {
    state.command.is_some() || state.snapshot.is_some()
}

pub(crate) fn merge_snapshot_command(
    pending: Option<SnapshotCommand>,
    snapshot: Value,
    mut pulses: Vec<RuntimeUiPulse>,
    mut rendered_acks: Vec<mpsc::Sender<Result<(), String>>>,
) -> Option<SnapshotCommand> {
    match pending {
        Some(SnapshotCommand {
            pulses: mut pending,
            rendered_acks: mut pending_acks,
            ..
        }) => {
            pending.append(&mut pulses);
            pending_acks.append(&mut rendered_acks);
            Some(SnapshotCommand {
                snapshot,
                pulses: pending,
                rendered_acks: pending_acks,
            })
        }
        None => Some(SnapshotCommand {
            snapshot,
            pulses,
            rendered_acks,
        }),
    }
}

pub(crate) fn reject_pending_command(state: &mut RenderState, message: &str) {
    if let Some(command) = state.command.take() {
        let ack = match command {
            RenderCommand::Snapshot { rendered_acks, .. } => {
                for ack in rendered_acks {
                    let _ = ack.send(Err(message.into()));
                }
                return;
            }
            RenderCommand::MarkFirstMenuRendered { ack }
            | RenderCommand::MarkFailed { ack }
            | RenderCommand::Abort { ack }
            | RenderCommand::Shutdown { ack } => ack,
            RenderCommand::Ownership {
                cancellation, ack, ..
            } => {
                cancellation.store(true, Ordering::Release);
                ack
            }
        };
        let _ = ack.send(Err(message.into()));
    }
    if let Some(snapshot) = state.snapshot.take() {
        for ack in snapshot.rendered_acks {
            let _ = ack.send(Err(message.into()));
        }
    }
}
