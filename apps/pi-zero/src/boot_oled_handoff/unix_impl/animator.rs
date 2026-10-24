use super::*;

pub(crate) struct AnimatorHandoff {
    pub(super) directory: HandoffDirectory,
    lock: File,
    boot_id: String,
    cycle_count: u64,
    request_id: Option<String>,
}

pub(crate) fn animator_start() -> Result<AnimatorHandoff, String> {
    animator_start_at(Path::new(HANDOFF_ROOT))
}

pub(super) fn animator_start_at(path: &Path) -> Result<AnimatorHandoff, String> {
    let directory = HandoffDirectory::open_existing_at(path)?;
    let lock = open_lock(&directory, true)?;
    flock(&lock, true)?;
    directory.validate_entries()?;
    match read_status(&directory)? {
        Some(status) if status.boot_id == directory.identity.boot_id => {
            return Err("OLED handoff already exists for this boot; refusing to clobber it".into())
        }
        Some(status) => {
            if let Some(stop) = read_stop(&directory)? {
                if stop.boot_id == directory.identity.boot_id {
                    return Err("OLED stop request already exists for this boot".into());
                }
                if stop.boot_id != status.boot_id {
                    return Err("OLED stale handoff entries belong to different boots".into());
                }
            }
            cleanup_previous_state(&directory)?;
        }
        None => {
            if let Some(stop) = read_stop(&directory)? {
                if stop.boot_id == directory.identity.boot_id {
                    return Err("OLED stop request already exists for this boot".into());
                }
                cleanup_previous_state(&directory)?;
            }
        }
    }
    cleanup_temporary_files(&directory)?;
    let boot_id = directory.identity.boot_id.clone();
    write_status(
        &directory,
        &HandoffStatus::new(HandoffPhase::Animating, boot_id.clone(), 0, None),
    )?;
    Ok(AnimatorHandoff {
        directory,
        lock,
        boot_id,
        cycle_count: 0,
        request_id: None,
    })
}

impl AnimatorHandoff {
    pub(crate) fn stop_requested(&mut self) -> Result<bool, String> {
        let Some(request) = read_stop(&self.directory)? else {
            return Ok(false);
        };
        if request.boot_id != self.boot_id {
            return Err("OLED stop request belongs to a different boot".into());
        }
        if let Some(existing) = &self.request_id {
            if existing != &request.request_id {
                return Err("OLED stop request changed during animation".into());
            }
        } else {
            self.request_id = Some(request.request_id.clone());
        }
        write_status(
            &self.directory,
            &HandoffStatus::new(
                HandoffPhase::ReleaseRequested,
                self.boot_id.clone(),
                self.cycle_count,
                self.request_id.clone(),
            ),
        )?;
        Ok(true)
    }

    pub(crate) fn publish_cycle(&mut self) -> Result<(), String> {
        if self.request_id.is_some() {
            return Err("OLED animation cannot publish a cycle after release was requested".into());
        }
        self.cycle_count = self.cycle_count.saturating_add(1);
        write_status(
            &self.directory,
            &HandoffStatus::new(
                HandoffPhase::Animating,
                self.boot_id.clone(),
                self.cycle_count,
                None,
            ),
        )
    }

    pub(crate) fn mark_failed(&mut self) {
        let status = HandoffStatus::new(
            HandoffPhase::Animating,
            self.boot_id.clone(),
            self.cycle_count,
            self.request_id.clone(),
        );
        let Ok(request_id) = create_or_attach_stop(&self.directory, &status) else {
            return;
        };
        self.request_id = Some(request_id.clone());
        let _ = write_status(
            &self.directory,
            &HandoffStatus::new(
                HandoffPhase::Failed,
                self.boot_id.clone(),
                self.cycle_count,
                Some(request_id),
            ),
        );
    }

    pub(crate) fn release(mut self) -> Result<(), String> {
        if self.request_id.is_none() {
            return Err("OLED animation release requires a validated stop request".into());
        }
        let result = write_status(
            &self.directory,
            &HandoffStatus::new(
                HandoffPhase::Released,
                self.boot_id.clone(),
                self.cycle_count,
                self.request_id.clone(),
            ),
        );
        if result.is_err() {
            self.mark_failed();
            return result;
        }
        let _ = self.lock.sync_all();
        Ok(())
    }
}
