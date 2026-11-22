use super::*;

impl UserDataTransferService {
    pub(crate) fn confirm_pending_restore(&self, approved: bool) -> bool {
        let pending = {
            let Ok(mut state) = self.inner.state.lock() else {
                return false;
            };
            if !state.active || self.inner.stop.load(Ordering::Acquire) {
                return false;
            }
            let pending = match std::mem::replace(&mut state.restore, RestoreState::None) {
                RestoreState::Pending(pending) => *pending,
                other => {
                    state.restore = other;
                    return false;
                }
            };
            super::queue_transfer_status(&mut state);
            if !approved {
                state.restore = RestoreState::Finished {
                    session: pending.session.clone(),
                    status: "cancelled",
                };
            } else {
                self.inner.store_write_barrier.invalidate();
                state.restore = RestoreState::Restoring {
                    session: pending.session.clone(),
                };
                super::queue_runtime_status(
                    &mut state,
                    RuntimeStoreResult::UserDataRestoreStatus {
                        status: RuntimeUserDataRestoreStatus {
                            phase: RuntimeUserDataRestorePhase::Restoring,
                        },
                    },
                );
            }
            pending
        };
        if !approved {
            remove_stage(&pending.staged);
            return true;
        }
        let session = pending.session.clone();
        let worker_session = session.clone();
        let stage_root = pending.staged.root.clone();
        let worker_stage_root = stage_root.clone();
        let inner = self.inner.clone();
        let Ok(mut worker_slot) = self.inner.restore_worker.lock() else {
            fail_restore_start(&self.inner, &stage_root, session);
            return true;
        };
        let worker = thread::Builder::new()
            .name("octessera-user-data-restore".into())
            .spawn(move || {
                let result = (|| {
                    drop(
                        inner
                            .store_lock
                            .lock()
                            .map_err(|_| "pi store is unavailable".to_string())?,
                    );
                    let preflight = inner
                        .restore_preflight
                        .lock()
                        .map_err(|_| "restore preflight is unavailable".to_string())?
                        .clone();
                    if let Some(preflight) = preflight {
                        preflight()?;
                    }
                    crate::user_data_restore::restore(
                        &crate::user_data_restore::LiveFilesystems,
                        &inner.store_lock,
                        crate::user_data_restore::UserDataDirs {
                            store: &inner.store_dir,
                            samples: &inner.samples_dir,
                            recordings: &inner.recordings_dir,
                            screen_recordings: &inner.screen_recordings_dir,
                        },
                        &worker_session,
                        pending.staged,
                    )
                })();
                if let Err(error) = &result {
                    eprintln!("restore failed: {error}");
                }
                inner.store_write_barrier.finish(result.is_ok());
                remove_stage_root(&worker_stage_root);
                let phase = restore_finished_phase(&result, &inner.store_dir, &worker_session);
                if let Ok(mut state) = inner.state.lock() {
                    state.restore = RestoreState::Finished {
                        session: worker_session,
                        status: restore_finished_status(&result),
                    };
                    super::queue_runtime_status(
                        &mut state,
                        RuntimeStoreResult::UserDataRestoreStatus {
                            status: RuntimeUserDataRestoreStatus { phase },
                        },
                    );
                }
            })
            .map_err(|_| "restore worker failed to start".to_string());
        match worker {
            Ok(join) => *worker_slot = Some(join),
            Err(_) => fail_restore_start(&self.inner, &stage_root, session),
        }
        true
    }
}

fn fail_restore_start(inner: &Arc<TransferInner>, stage_root: &Path, session: String) {
    remove_stage_root(stage_root);
    inner.store_write_barrier.finish(false);
    if let Ok(mut state) = inner.state.lock() {
        state.restore = RestoreState::Finished {
            session,
            status: "failed",
        };
        super::queue_runtime_status(
            &mut state,
            RuntimeStoreResult::UserDataRestoreStatus {
                status: RuntimeUserDataRestoreStatus {
                    phase: RuntimeUserDataRestorePhase::FailedUnchanged,
                },
            },
        );
    }
}

fn restore_finished_phase(
    result: &Result<(), String>,
    store_dir: &Path,
    session: &str,
) -> RuntimeUserDataRestorePhase {
    let pre_restore_kept = || {
        store_dir.parent().is_some_and(|parent| {
            let staging = parent.join(crate::user_data_restore::STAGING_DIR_NAME);
            crate::user_data_restore::pre_restore_backup_path(&staging, session).exists()
        })
    };
    match result {
        Ok(()) => RuntimeUserDataRestorePhase::Succeeded,
        Err(error) if error == crate::user_data_restore::EARLIER_RESTORE_UNFINISHED => {
            RuntimeUserDataRestorePhase::EarlierRestoreUnfinished
        }
        Err(_) if pre_restore_kept() => RuntimeUserDataRestorePhase::Failed,
        Err(_) => RuntimeUserDataRestorePhase::FailedUnchanged,
    }
}

fn restore_finished_status(result: &Result<(), String>) -> &'static str {
    if result.is_ok() {
        "restored"
    } else if result
        .as_ref()
        .err()
        .is_some_and(|error| error.contains("audio recording is active"))
    {
        "blocked_recording_active"
    } else if result
        .as_ref()
        .err()
        .is_some_and(|error| error == crate::user_data_restore::EARLIER_RESTORE_UNFINISHED)
    {
        "blocked_earlier_restore"
    } else {
        "failed"
    }
}
