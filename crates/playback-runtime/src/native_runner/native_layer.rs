use platform_core::NativeLayerEngine;
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) struct NativeLayer {
    pub(super) behavior_id: String,
    pub(super) name: String,
    pub(super) auto_name: bool,
    pub(super) seeded: bool,
    pub(super) behavior_config: Value,
    pub(super) behavior_config_history: BTreeMap<String, Value>,
    pub(super) engine: NativeLayerEngine,
}

impl NativeLayer {
    pub(super) fn new(
        behavior_id: &str,
        behavior_config: Value,
        engine: NativeLayerEngine,
    ) -> Self {
        Self {
            behavior_id: behavior_id.into(),
            name: super::behavior_menu::behavior_display_label(behavior_id),
            auto_name: true,
            seeded: false,
            behavior_config,
            behavior_config_history: BTreeMap::new(),
            engine,
        }
    }
}
