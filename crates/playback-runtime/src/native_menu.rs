#[cfg(test)]
use crate::protocol::SyncSource;
use binding_picker::{axis_binding_label, parameter_picker_group};
use bindings::xy_pad_items;
#[cfg(test)]
use fx::default_fx_bus_config;
#[cfg(test)]
use options::{FX_BUS_SLOT_OPTIONS, GLOBAL_FX_SLOT_OPTIONS};
#[cfg(test)]
use platform_core::{BUS_COUNT as FX_BUS_COUNT, GLOBAL_FX_SLOT_COUNT};
#[cfg(test)]
use pulses::default_pulses_layer_config;

mod binding_behavior;
mod binding_picker;
mod binding_picker_voice;
mod binding_pulses;
mod binding_pulses_axis;
mod binding_tree;
mod bindings;
mod format;
mod format_values;
mod fx;
mod fx_params;
mod help;
mod item_builders;
mod model;
mod model_binding_specs;
mod model_current;
mod model_edit;
mod model_navigation_memory;
mod model_root;
mod model_search;
mod model_snapshot;
mod model_values;
mod options;
mod pulses;
mod pulses_axis;
mod pulses_sections;
mod sample_browser_menu;
mod sparks;
mod synth_preset_items;
mod system;
mod system_aux;
mod system_saves;
mod types;
mod types_config;
mod voice;
mod voice_config_read;
mod voice_env_groups;

pub(crate) use fx::{fx_bus_slot_children_for_key, global_fx_slot_children_for_key};
pub(in crate::native_menu) use item_builders::*;
pub use model::NativeMenuPressResult;
pub(crate) use options::{is_valid_fx_bus_slot_type, is_valid_global_fx_slot_type};
pub use types::*;

use model_root::build_root;

#[cfg(test)]
mod tests;
