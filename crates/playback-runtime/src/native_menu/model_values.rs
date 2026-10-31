use crate::protocol::SyncSource;

use super::format::note_unit_to_pulses;
use super::section_labels::PLAY_PATH_PREFIX;
use super::{NativeMenuItem, NativeMenuModel, NativeMenuValue};

impl NativeMenuModel {
    pub fn selected_behavior(&self) -> Option<String> {
        self.value_for_key("behaviorId")
    }

    pub fn selected_algorithm_step_pulses(&self) -> Option<u32> {
        self.value_for_key("algorithmStep")
            .and_then(|value| note_unit_to_pulses(&value))
    }

    pub fn selected_sync_source(&self) -> Option<SyncSource> {
        match self.value_for_key("midiSyncMode")?.as_str() {
            "external" => Some(SyncSource::External),
            _ => Some(SyncSource::Internal),
        }
    }

    pub fn selected_master_volume(&self) -> Option<u8> {
        self.number_for_key("masterVolume")
            .map(|value| value.clamp(0, 100) as u8)
    }

    pub fn selected_display_brightness(&self) -> Option<u8> {
        self.number_for_key("displayBrightness")
            .map(|value| value.clamp(0, 100) as u8)
    }

    pub fn selected_button_brightness(&self) -> Option<u8> {
        self.number_for_key("buttonBrightness")
            .map(|value| value.clamp(0, 100) as u8)
    }

    pub fn selected_play_mode(&self) -> Option<String> {
        let path = self.current_focus_path();
        if path.starts_with(&format!("{PLAY_PATH_PREFIX} > Mix")) {
            return Some("mix".into());
        }
        if path.starts_with(&format!("{PLAY_PATH_PREFIX} > Pan")) {
            return Some("pan".into());
        }
        if path.starts_with(&format!("{PLAY_PATH_PREFIX} > FX")) {
            return Some("fx".into());
        }
        if path.starts_with(&format!("{PLAY_PATH_PREFIX} > Trigger Gate")) {
            return Some("trigger-gate".into());
        }
        if path.starts_with(&format!("{PLAY_PATH_PREFIX} > Transpose")) {
            return Some("transpose".into());
        }
        if path.starts_with(&format!("{PLAY_PATH_PREFIX} > XY")) {
            return Some("xy".into());
        }
        None
    }
}

pub(super) fn value_from_item(item: &NativeMenuItem) -> Option<String> {
    match &item.value {
        NativeMenuValue::Enum { options, selected } => options.get(*selected).cloned(),
        NativeMenuValue::Bool { value } => Some(if *value {
            "true".into()
        } else {
            "false".into()
        }),
        NativeMenuValue::Text { value, .. } => Some(value.clone()),
        _ => None,
    }
}

pub(super) fn number_from_item(item: &NativeMenuItem) -> Option<i32> {
    match &item.value {
        NativeMenuValue::Number { value, .. } => Some(*value),
        _ => None,
    }
}
