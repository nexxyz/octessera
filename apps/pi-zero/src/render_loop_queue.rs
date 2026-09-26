use crate::oled_frame_cache::OledFramePublication;
use crate::render::OledOwnershipStage;
use playback_runtime::{
    oled_frame::{OledPresentationMetrics, OledRuntimeErrorMetadata},
    PresentationScene,
};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;

pub(crate) enum RenderCommand {
    Snapshot {
        snapshot: Value,
        oled: OledFramePublication,
        rendered_acks: Vec<mpsc::Sender<Result<(), String>>>,
    },
    NativeSnapshot(Box<NativeSceneCommand>),
    MarkFirstMenuRendered {
        ack: mpsc::Sender<Result<(), String>>,
    },
    MarkFailed {
        ack: mpsc::Sender<Result<(), String>>,
    },
    Shutdown {
        ack: mpsc::Sender<Result<(), String>>,
    },
    PreserveTerminal {
        snapshot: Value,
        oled: OledFramePublication,
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

pub(crate) struct NativeSceneCompletion {
    pub(crate) generation: u64,
    pub(crate) result: Result<(), String>,
    pub(crate) frame_revision: Option<u64>,
}

pub(crate) struct NativeSceneCommand {
    pub(crate) scene: PresentationScene,
    pub(crate) metrics: OledPresentationMetrics,
    pub(crate) error: Option<OledRuntimeErrorMetadata>,
    pub(crate) completion: mpsc::Sender<NativeSceneCompletion>,
}

pub(crate) enum SnapshotCommand {
    Legacy {
        snapshot: Value,
        oled: OledFramePublication,
        rendered_acks: Vec<mpsc::Sender<Result<(), String>>>,
    },
    Native(Box<NativeSceneCommand>),
}

#[derive(Default)]
pub(crate) struct RenderState {
    pub(crate) command: Option<RenderCommand>,
    pub(crate) snapshot: Option<SnapshotCommand>,
    pub(crate) acknowledged_snapshot_published: bool,
    pub(crate) acknowledged_snapshot_rendered: bool,
    pub(crate) startup_accepted_oled_frame: Option<(u64, std::sync::Arc<[u8]>)>,
    pub(crate) recording_audio: Option<crate::audio::AudioService>,
    #[cfg(test)]
    pub(crate) native_render_gate: Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>,
    #[cfg(test)]
    pub(crate) fail_next_startup_oled_write: bool,
}

pub(crate) fn pending_work_wins_over_expired_animation_deadline(state: &RenderState) -> bool {
    state.command.is_some() || state.snapshot.is_some()
}

pub(crate) fn merge_snapshot_command(
    pending: Option<SnapshotCommand>,
    snapshot: Value,
    oled: OledFramePublication,
    mut rendered_acks: Vec<mpsc::Sender<Result<(), String>>>,
) -> Option<SnapshotCommand> {
    match pending {
        Some(SnapshotCommand::Legacy {
            rendered_acks: mut pending_acks,
            ..
        }) => {
            pending_acks.append(&mut rendered_acks);
            Some(SnapshotCommand::Legacy {
                snapshot,
                oled,
                rendered_acks: pending_acks,
            })
        }
        Some(SnapshotCommand::Native(previous)) => {
            supersede_native(previous, "native scene superseded by legacy snapshot");
            Some(SnapshotCommand::Legacy {
                snapshot,
                oled,
                rendered_acks,
            })
        }
        None => Some(SnapshotCommand::Legacy {
            snapshot,
            oled,
            rendered_acks,
        }),
    }
}

pub(crate) fn merge_native_scene(state: &mut RenderState, command: NativeSceneCommand) {
    if let Some(previous) = state.snapshot.take() {
        match previous {
            SnapshotCommand::Native(previous) => {
                supersede_native(previous, "native scene superseded")
            }
            SnapshotCommand::Legacy { rendered_acks, .. } => {
                for ack in rendered_acks {
                    let _ = ack.send(Err("legacy snapshot superseded by native scene".into()));
                }
            }
        }
    }
    state.snapshot = Some(SnapshotCommand::Native(Box::new(command)));
}

pub(crate) fn supersede_native(command: Box<NativeSceneCommand>, reason: &str) {
    let _ = command.completion.send(NativeSceneCompletion {
        generation: command.scene.generation(),
        result: Err(reason.into()),
        frame_revision: None,
    });
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
            RenderCommand::NativeSnapshot(scene) => {
                supersede_native(scene, message);
                return;
            }
            RenderCommand::MarkFirstMenuRendered { ack }
            | RenderCommand::MarkFailed { ack }
            | RenderCommand::Abort { ack }
            | RenderCommand::Shutdown { ack }
            | RenderCommand::PreserveTerminal { ack, .. } => ack,
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
        match snapshot {
            SnapshotCommand::Legacy { rendered_acks, .. } => {
                for ack in rendered_acks {
                    let _ = ack.send(Err(message.into()));
                }
            }
            SnapshotCommand::Native(scene) => supersede_native(scene, message),
        }
    }
}
