use serde_json::Value;
use std::sync::Arc;

pub(super) mod write;
pub(super) use write::{DefaultWrite, DefaultWriteCompletion};

/// Perf. Mode is the one System setting that applies only after the app restarts.
const RESTART_SETTING_KEY: &str = "sound.optimizeFor";
const RESTART_SETTING_PATH: (&str, &str) = ("sound", "optimizeFor");

pub(super) fn is_restart_setting(key: &str) -> bool {
    key == RESTART_SETTING_KEY
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DefaultSaveScope {
    Ordinary,
    Autosave,
    RestartSetting,
    RestartEverything,
}

impl DefaultSaveScope {
    pub(super) fn is_restart(self) -> bool {
        matches!(self, Self::RestartSetting | Self::RestartEverything)
    }

    pub(super) fn is_patch(self) -> bool {
        matches!(self, Self::Ordinary | Self::Autosave)
    }
}

#[derive(Clone, Debug, PartialEq)]
enum RestartFlow {
    Idle,
    SaveChoice { setting_payload: Option<Value> },
    Saving,
    RestartChoice,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RestartSettingsState {
    pub(super) persisted_default: Arc<Value>,
    pending_write: Option<DefaultWrite>,
    native_payload_missing: bool,
    editing: Option<Value>,
    flow: RestartFlow,
}

impl Default for RestartSettingsState {
    fn default() -> Self {
        Self {
            persisted_default: Arc::new(Value::Null),
            pending_write: None,
            native_payload_missing: false,
            editing: None,
            flow: RestartFlow::Idle,
        }
    }
}

impl RestartSettingsState {
    pub(super) fn new(persisted_default: Value) -> Self {
        Self {
            persisted_default: Arc::new(persisted_default),
            ..Self::default()
        }
    }

    pub(super) fn update_system_baseline(&mut self, payload: Arc<Value>) {
        self.persisted_default = payload;
    }

    pub(super) fn has_pending_patch_write(&self) -> bool {
        self.pending_write
            .as_ref()
            .is_some_and(|write| write.scope.is_patch())
    }

    pub(super) fn pending_write_scope(&self) -> Option<DefaultSaveScope> {
        self.pending_write.as_ref().map(|write| write.scope)
    }

    pub(super) fn has_pending_write(&self) -> bool {
        self.pending_write.is_some()
    }

    pub(super) fn setting_payload(&self, current: &Value) -> Option<Value> {
        let (parent, leaf) = RESTART_SETTING_PATH;
        let value = setting_value(current)?.clone();
        let mut payload = (*self.persisted_default).clone();
        let runtime = payload.get_mut("runtimeConfig")?.as_object_mut()?;
        let group = runtime.get_mut(parent)?.as_object_mut()?;
        group.insert(leaf.into(), value);
        Some(payload)
    }

    pub(super) fn begin_edit(&mut self, current: &Value) {
        self.editing = setting_value(current).cloned();
    }

    pub(super) fn finish_edit(&mut self, current: &Value) -> bool {
        self.editing
            .take()
            .is_some_and(|initial| setting_value(current).is_some_and(|value| value != &initial))
    }

    pub(super) fn is_editing(&self) -> bool {
        self.editing.is_some()
    }

    pub(super) fn open_save_choice(&mut self, setting_payload: Option<Value>) {
        self.flow = RestartFlow::SaveChoice { setting_payload };
    }

    pub(super) fn save_choice_setting_payload(&self) -> Option<Value> {
        match &self.flow {
            RestartFlow::SaveChoice { setting_payload } => setting_payload.clone(),
            _ => None,
        }
    }

    pub(super) fn update_save_choice_setting_payload(&mut self, setting_payload: Option<Value>) {
        if let RestartFlow::SaveChoice {
            setting_payload: current,
        } = &mut self.flow
        {
            *current = setting_payload;
        }
    }

    pub(super) fn is_save_choice(&self) -> bool {
        matches!(self.flow, RestartFlow::SaveChoice { .. })
    }

    pub(super) fn abandon_pending_write(&mut self) {
        self.pending_write = None;
        self.native_payload_missing = false;
    }

    pub(super) fn is_saving(&self) -> bool {
        matches!(self.flow, RestartFlow::Saving)
    }

    pub(super) fn is_restart_choice(&self) -> bool {
        matches!(self.flow, RestartFlow::RestartChoice)
    }

    pub(super) fn continue_after_save(&mut self) {
        self.editing = None;
        self.flow = RestartFlow::Idle;
    }

    pub(super) fn cancel(&mut self) {
        self.editing = None;
        self.flow = RestartFlow::Idle;
    }
}

fn setting_value(payload: &Value) -> Option<&Value> {
    let (parent, leaf) = RESTART_SETTING_PATH;
    payload
        .get("runtimeConfig")
        .and_then(|runtime| runtime.get(parent))
        .and_then(|group| group.get(leaf))
}
