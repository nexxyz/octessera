use serde_json::Value;
use std::sync::Arc;

use super::UsbDataRole;

pub(super) mod write;
pub(super) use write::{DefaultWrite, DefaultWriteCompletion};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RestartSetting {
    AudioOutputDac,
    AudioOutputUsb,
    AudioOutputHdmi,
    UsbMidiOut,
    UsbDataRole,
    AudioOutputBufferFrames,
    AudioOptimization,
}

impl RestartSetting {
    pub(super) fn from_key(key: &str) -> Option<Self> {
        match key {
            "audioOutputs.dac" => Some(Self::AudioOutputDac),
            "audioOutputs.usb" => Some(Self::AudioOutputUsb),
            "audioOutputs.hdmi" => Some(Self::AudioOutputHdmi),
            "usb.midiOutEnabled" => Some(Self::UsbMidiOut),
            "usb.dataRole" => Some(Self::UsbDataRole),
            "sound.audioOutputBufferFrames" => Some(Self::AudioOutputBufferFrames),
            "sound.optimizeFor" => Some(Self::AudioOptimization),
            _ => None,
        }
    }

    fn path(self) -> (&'static str, &'static str) {
        match self {
            Self::AudioOutputDac => ("audioOutputs", "dac"),
            Self::AudioOutputUsb => ("audioOutputs", "usb"),
            Self::AudioOutputHdmi => ("audioOutputs", "hdmi"),
            Self::UsbMidiOut => ("usb", "midiOutEnabled"),
            Self::UsbDataRole => ("usb", "dataRole"),
            Self::AudioOutputBufferFrames => ("sound", "audioOutputBufferFrames"),
            Self::AudioOptimization => ("sound", "optimizeFor"),
        }
    }
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
struct RestartEdit {
    setting: RestartSetting,
    initial_value: Value,
}

#[derive(Clone, Debug, PartialEq)]
enum RestartFlow {
    Idle,
    SaveChoice {
        setting: RestartSetting,
        setting_payload: Option<Value>,
    },
    Saving,
    RestartChoice,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RestartSettingsState {
    pub(super) persisted_default: Arc<Value>,
    pending_write: Option<DefaultWrite>,
    native_payload_missing: bool,
    editing: Option<RestartEdit>,
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

    pub(super) fn setting_payload(
        &self,
        current: &Value,
        setting: RestartSetting,
    ) -> Option<Value> {
        if setting == RestartSetting::UsbDataRole {
            return self.usb_data_role_setting_payload(current);
        }
        let (parent, leaf) = setting.path();
        let value = current
            .get("runtimeConfig")
            .and_then(|runtime| runtime.get(parent))
            .and_then(|group| group.get(leaf))?
            .clone();
        let mut payload = (*self.persisted_default).clone();
        let runtime = payload.get_mut("runtimeConfig")?.as_object_mut()?;
        let group = runtime.get_mut(parent)?.as_object_mut()?;
        group.insert(leaf.into(), value);
        Some(payload)
    }

    fn usb_data_role_setting_payload(&self, current: &Value) -> Option<Value> {
        let current_runtime = current.get("runtimeConfig")?;
        let mut payload = (*self.persisted_default).clone();
        let runtime = payload.get_mut("runtimeConfig")?.as_object_mut()?;
        for (parent, leaf) in [
            ("usb", "dataRole"),
            ("usb", "midiOutEnabled"),
            ("audioOutputs", "usb"),
        ] {
            let value = current_runtime.get(parent)?.get(leaf)?.clone();
            let group = runtime.get_mut(parent)?.as_object_mut()?;
            group.insert(leaf.into(), value);
        }
        Some(payload)
    }

    pub(super) fn begin_edit(&mut self, current: &Value, setting: RestartSetting) {
        self.editing = setting_value(current, setting)
            .cloned()
            .map(|initial_value| RestartEdit {
                setting,
                initial_value,
            });
    }

    pub(super) fn finish_edit(&mut self, current: &Value, setting: RestartSetting) -> bool {
        let Some(edit) = self.editing.take() else {
            return false;
        };
        edit.setting == setting
            && setting_value(current, setting).is_some_and(|value| value != &edit.initial_value)
    }

    pub(super) fn is_editing(&self) -> bool {
        self.editing.is_some()
    }

    pub(super) fn open_save_choice(
        &mut self,
        setting: RestartSetting,
        setting_payload: Option<Value>,
    ) {
        self.flow = RestartFlow::SaveChoice {
            setting,
            setting_payload,
        };
    }

    pub(super) fn save_choice_setting_payload(&self) -> Option<Value> {
        match &self.flow {
            RestartFlow::SaveChoice {
                setting_payload, ..
            } => setting_payload.clone(),
            _ => None,
        }
    }

    pub(super) fn save_choice_setting(&self) -> Option<RestartSetting> {
        match self.flow {
            RestartFlow::SaveChoice { setting, .. } => Some(setting),
            _ => None,
        }
    }

    pub(super) fn update_save_choice_setting_payload(&mut self, setting_payload: Option<Value>) {
        if let RestartFlow::SaveChoice {
            setting_payload: current,
            ..
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

fn setting_value(payload: &Value, setting: RestartSetting) -> Option<&Value> {
    let (parent, leaf) = setting.path();
    payload
        .get("runtimeConfig")
        .and_then(|runtime| runtime.get(parent))
        .and_then(|group| group.get(leaf))
}

fn payload_is_host(payload: &Value) -> bool {
    payload
        .get("runtimeConfig")
        .and_then(|runtime| runtime.get("usb"))
        .and_then(|usb| usb.get("dataRole"))
        .and_then(Value::as_str)
        .is_some_and(|role| role == UsbDataRole::Host.as_str())
}
