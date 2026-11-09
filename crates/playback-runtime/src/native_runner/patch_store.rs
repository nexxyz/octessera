use std::time::Instant;

/// The patch store as the runner sees it: the saved patch names, the loaded one, the name being
/// drafted or renamed, and when the last rolling backup was saved.
#[derive(Clone, Debug)]
pub(super) struct NativePatchStore {
    pub(super) preset_names: Vec<String>,
    pub(super) current_preset_name: Option<String>,
    pub(super) preset_draft_name: String,
    pub(super) preset_rename_source: Option<String>,
    pub(super) last_backup_save_at: Option<Instant>,
}

impl NativePatchStore {
    pub(super) fn new(preset_names: Vec<String>) -> Self {
        Self {
            preset_names,
            current_preset_name: None,
            preset_draft_name: crate::fresh_preset_name(),
            preset_rename_source: None,
            last_backup_save_at: None,
        }
    }
}
