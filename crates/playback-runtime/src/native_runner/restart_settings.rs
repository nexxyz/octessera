use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RestartSetting {
    AudioOutputDac,
    AudioOutputUsb,
    AudioOutputHdmi,
    UsbMidiOut,
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
            "sound.audioOutputBufferFrames" => Some(Self::AudioOutputBufferFrames),
            "sound.optimizeFor" => Some(Self::AudioOptimization),
            _ => None,
        }
    }

    pub(super) fn is_audio_output(self) -> bool {
        matches!(
            self,
            Self::AudioOutputDac | Self::AudioOutputUsb | Self::AudioOutputHdmi
        )
    }

    fn path(self) -> (&'static str, &'static str) {
        match self {
            Self::AudioOutputDac => ("audioOutputs", "dac"),
            Self::AudioOutputUsb => ("audioOutputs", "usb"),
            Self::AudioOutputHdmi => ("audioOutputs", "hdmi"),
            Self::UsbMidiOut => ("usb", "midiOutEnabled"),
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
    fn is_restart(self) -> bool {
        matches!(self, Self::RestartSetting | Self::RestartEverything)
    }

    pub(super) fn mode(self) -> Option<String> {
        match self {
            Self::Ordinary => None,
            Self::Autosave => Some("deferred".into()),
            Self::RestartSetting => Some("restart-setting".into()),
            Self::RestartEverything => Some("restart-everything".into()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct DefaultWrite {
    revision: u64,
    payload: Value,
    scope: DefaultSaveScope,
    request_id: Option<String>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DefaultWriteCompletion {
    pub(super) scope: DefaultSaveScope,
    pub(super) restart_flow: bool,
    pub(super) succeeded: bool,
}

#[derive(Clone, Debug)]
pub(super) struct RestartSettingsState {
    pub(super) persisted_default: Value,
    pending_write: Option<DefaultWrite>,
    restart_after_pending_write: bool,
    editing: Option<RestartEdit>,
    flow: RestartFlow,
}

impl Default for RestartSettingsState {
    fn default() -> Self {
        Self {
            persisted_default: Value::Null,
            pending_write: None,
            restart_after_pending_write: false,
            editing: None,
            flow: RestartFlow::Idle,
        }
    }
}

impl RestartSettingsState {
    pub(super) fn new(persisted_default: Value) -> Self {
        Self {
            persisted_default,
            ..Self::default()
        }
    }

    pub(super) fn set_baseline(&mut self, payload: Value) {
        self.persisted_default = payload;
        self.abandon_pending_write();
        self.editing = None;
        self.flow = RestartFlow::Idle;
    }

    pub(super) fn has_pending_write(&self) -> bool {
        self.pending_write.is_some()
    }

    pub(super) fn setting_payload(
        &self,
        current: &Value,
        setting: RestartSetting,
        revision: u64,
    ) -> Option<Value> {
        let (parent, leaf) = setting.path();
        let value = current
            .get("runtimeConfig")
            .and_then(|runtime| runtime.get(parent))
            .and_then(|group| group.get(leaf))?
            .clone();
        let mut payload = self.persisted_default.clone();
        let runtime = payload.get_mut("runtimeConfig")?.as_object_mut()?;
        let group = runtime.get_mut(parent)?.as_object_mut()?;
        group.insert(leaf.into(), value);
        payload
            .as_object_mut()?
            .insert("revision".into(), json!(revision));
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

    pub(super) fn start_write(
        &mut self,
        payload: Value,
        scope: DefaultSaveScope,
        revision: u64,
    ) -> bool {
        if self.has_pending_write() {
            return false;
        }
        self.pending_write = Some(DefaultWrite {
            revision,
            payload,
            scope,
            request_id: None,
        });
        if scope.is_restart() {
            self.flow = RestartFlow::Saving;
        }
        true
    }

    pub(super) fn defer_restart_after_pending_write(&mut self) {
        self.restart_after_pending_write = true;
    }

    pub(super) fn abandon_pending_write(&mut self) {
        self.pending_write = None;
        self.restart_after_pending_write = false;
    }

    pub(super) fn take_restart_after_pending_write(&mut self) -> bool {
        std::mem::take(&mut self.restart_after_pending_write)
    }

    pub(super) fn track_write(
        &mut self,
        payload: Value,
        scope: DefaultSaveScope,
        revision: u64,
    ) -> bool {
        if self.has_pending_write() {
            return false;
        }
        self.pending_write = Some(DefaultWrite {
            revision,
            payload,
            scope,
            request_id: None,
        });
        true
    }

    pub(super) fn register_request(&mut self, request_id: &str, revision: Option<u64>) {
        let Some(write) = self.pending_write.as_mut() else {
            return;
        };
        if revision == Some(write.revision) {
            write.request_id = Some(request_id.into());
        }
    }

    pub(super) fn pending_write_revision(&self) -> Option<u64> {
        self.pending_write.as_ref().map(|write| write.revision)
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

    pub(super) fn acknowledge_write(
        &mut self,
        request_id: &str,
        revision: Option<u64>,
        succeeded: bool,
    ) -> Option<DefaultWriteCompletion> {
        let revision = revision?;
        let write = self.pending_write.as_mut()?;
        if write.revision != revision || write.request_id.as_deref() != Some(request_id) {
            return None;
        }
        let write = self.pending_write.take()?;
        let restart_flow = write.scope.is_restart() && self.is_saving();
        if succeeded {
            self.persisted_default = write.payload;
        }
        if restart_flow {
            self.flow = if succeeded {
                RestartFlow::RestartChoice
            } else {
                RestartFlow::Idle
            };
        }
        Some(DefaultWriteCompletion {
            scope: write.scope,
            restart_flow,
            succeeded,
        })
    }
}

fn setting_value(payload: &Value, setting: RestartSetting) -> Option<&Value> {
    let (parent, leaf) = setting.path();
    payload
        .get("runtimeConfig")
        .and_then(|runtime| runtime.get(parent))
        .and_then(|group| group.get(leaf))
}
