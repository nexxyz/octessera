use super::pi_pending_persistence::{NativePiSave, PendingPiPersistence};
use super::platform_native_persistence::{failure, NativeAutosaveWrite, PlatformWorkItem};
use super::PiPlatformService;
use playback_runtime::{
    HostMessage, NativeConfigSnapshot, NativeRunner, PlaybackRuntime, RuntimeOperation,
};
use std::time::Instant;

struct NativeAutosavePlan {
    snapshot: NativeConfigSnapshot,
    pending: NativePiSave,
    default: bool,
    backup: bool,
    now: Instant,
}

pub(crate) fn flush_due_native_persistence(
    pending: &mut PendingPiPersistence,
    service: &PiPlatformService,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    now: Instant,
) -> Vec<HostMessage> {
    let generation = service.store_write_generation();
    let blocked = service.store_writes_blocked();
    let intent = if blocked {
        None
    } else {
        runner.persistence_intent_at(now)
    };
    pending.observe_native(intent, now, generation, blocked);
    let Some(due) = pending.due_native(now) else {
        return Vec::new();
    };
    let Some(intent) = intent.filter(|intent| intent.revision() == due.revision) else {
        return Vec::new();
    };
    let default = due.default_eligible
        && due.default_due_at.is_some_and(|due_at| due_at <= now)
        && intent.default_eligible()
        && service.native_default_write().is_none();
    let backup = due.backup_eligible && intent.backup_eligible();
    if !default && !backup {
        return Vec::new();
    }
    #[cfg(test)]
    pending.record_snapshot_capture();
    let snapshot = runner.capture_config_snapshot();
    assert_eq!(snapshot.revision(), due.revision);
    submit_native_autosave(
        pending,
        service,
        playback,
        runner,
        NativeAutosavePlan {
            snapshot,
            pending: due,
            default,
            backup,
            now,
        },
    )
}

fn submit_native_autosave(
    pending: &mut PendingPiPersistence,
    service: &PiPlatformService,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    plan: NativeAutosavePlan,
) -> Vec<HostMessage> {
    let NativeAutosavePlan {
        snapshot,
        pending: due,
        default,
        backup,
        now,
    } = plan;
    let default_request = default.then(|| {
        playback.next_native_store_request(RuntimeOperation::StoreSaveDefault, due.revision)
    });
    let backup_request = backup.then(|| {
        playback.next_native_store_request(RuntimeOperation::StoreSaveBackup, due.revision)
    });
    if let Some(request) = &default_request {
        if !runner.register_native_default_write(request.request_id(), request.revision(), true) {
            pending.mark_submitted(due.revision, due.generation, true, false);
            return vec![failure(
                request,
                "native default save registration failed".into(),
            )];
        }
    }
    match service.enqueue_native_autosave(NativeAutosaveWrite {
        default: default_request.clone(),
        backup: backup_request.clone(),
        snapshot: Box::new(snapshot),
        generation: due.generation,
    }) {
        Ok(()) => {
            if backup {
                runner.mark_native_backup_issued_at(now);
            }
            pending.mark_submitted(due.revision, due.generation, default, backup);
            Vec::new()
        }
        Err(message) => {
            pending.mark_submitted(due.revision, due.generation, default, backup);
            default_request
                .iter()
                .chain(backup_request.iter())
                .map(|request| failure(request, message.clone()))
                .collect()
        }
    }
}

pub(crate) fn take_manual_save(
    pending: &mut PendingPiPersistence,
    service: &PiPlatformService,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
) -> Option<HostMessage> {
    let manual = runner.take_manual_save_request()?;
    pending.cancel();
    super::platform_native_persistence::submit_manual_save(service, playback, runner, manual)
}

impl PiPlatformService {
    fn enqueue_native_autosave(&self, write: NativeAutosaveWrite) -> Result<(), String> {
        if write.default.is_none() && write.backup.is_none() {
            return Err("native autosave has no eligible operation".into());
        }
        if self.store_write_barrier.is_blocked() {
            return Err("restore is awaiting restored-state acknowledgement".into());
        }
        let mut current = self
            .native_default_write
            .lock()
            .map_err(|_| "native default save state is unavailable".to_string())?;
        if write.default.is_some() && current.is_some() {
            return Err("native default save is already pending".into());
        }
        if let Some(request) = &write.default {
            *current = Some(request.clone());
        }
        let has_default = write.default.is_some();
        self.jobs
            .try_send(PlatformWorkItem::NativeAutosave { write })
            .map_err(|error| {
                if has_default {
                    *current = None;
                }
                match error {
                    std::sync::mpsc::TrySendError::Full(_) => {
                        "pi platform service queue is full".to_string()
                    }
                    std::sync::mpsc::TrySendError::Disconnected(_) => {
                        "pi platform service stopped".to_string()
                    }
                }
            })
    }
}

#[cfg(test)]
mod tests;
