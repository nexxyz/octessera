use super::defaults::default_link_layer;
use super::state_link::{NativeLinkLayer, NativeParamMods};
use super::trigger_probability::{TileProbabilityMap, FULL_PROBABILITY_MAP};
use super::TriggerProbabilityMode;
use platform_core::NativeLayerEngine;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct NativeLayer {
    pub(super) behavior_id: String,
    pub(super) name: String,
    pub(super) auto_name: bool,
    pub(super) seeded: bool,
    pub(super) behavior_config: Value,
    pub(super) behavior_config_history: BTreeMap<String, Value>,
    pub(super) engine: NativeLayerEngine,
    pub(super) link: NativeLinkLayer,
    pub(super) param_mods: NativeParamMods,
    pub(super) trigger_probability_map: TileProbabilityMap,
    pub(super) trigger_gate_restore_mode: Option<TriggerProbabilityMode>,
    pub(super) emitted_note_owners: BTreeSet<(bool, u8, u8)>,
    pub(super) emitted_tile_owners: BTreeSet<(u8, u8)>,
}

impl NativeLayer {
    pub(super) fn new(
        index: usize,
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
            link: default_link_layer(index),
            param_mods: NativeParamMods::default(),
            trigger_probability_map: FULL_PROBABILITY_MAP,
            trigger_gate_restore_mode: None,
            emitted_note_owners: BTreeSet::new(),
            emitted_tile_owners: BTreeSet::new(),
        }
    }
}
