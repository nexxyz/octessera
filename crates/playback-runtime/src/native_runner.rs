use crate::native_menu::{
    NativeAuxBindingConfig, NativeFxBusConfig, NativeMenuAction, NativeMenuConfig, NativeMenuModel,
    NativeParamBindingSpec, NativeParamModsConfig, NativePulsesLayerConfig, NativeValueLaneConfig,
};
#[cfg(test)]
use crate::protocol::{HostMessage, RunnerMessage, RuntimeAudioCommand, RuntimeStoreResult};
use crate::protocol::{
    MidiPort, RuntimeMomentaryFxTarget, RuntimePlatformEffect, RuntimeTransportState, SampleEntry,
    SyncSource,
};
use crate::runtime::{CoreRunner, RuntimeConfig};
use defaults::{
    default_fx_buses, default_global_fx_params, default_global_fx_slots, default_instruments,
    default_pulses_layers, derive_bus_name, derive_instrument_name, fx_default_params,
    fx_slot_payload_with_params, legacy_derive_bus_name, legacy_derive_instrument_name,
    note_unit_from_pulses, note_unit_to_pulses,
};
use modulation_keys::{parse_instrument_binding_key, parse_layer_behavior_config_binding_key};
#[cfg(test)]
use modulation_sampler::sampler_assignment_velocity;
use platform_core::{
    default_mapping_config, AxisStrategy, BehaviorActionInput, BehaviorConfigItem,
    BehaviorConfigItemType, DeviceInput, GlobalSoundConfig, GridInteraction, InterpretationProfile,
    NativeBehavior, NativeLayerEngine, NativeLayerEngineConfig, NoteBehavior, RangeMode,
    TickStrategy, TriggerAction, TriggerTarget, VelocityCurve, BUS_COUNT, GLOBAL_FX_SLOT_COUNT,
    GRID_HEIGHT, GRID_WIDTH, INSTRUMENT_COUNT, LAYER_COUNT, PAN_POSITION_COUNT, SAMPLE_SLOT_COUNT,
    SPARKS_FX_MAX_CONCURRENT,
};
#[cfg(test)]
use platform_core::{CellTriggerIntent, MusicalEvent};
use serde_json::{json, Value};
use sparks_fx_utils::{
    default_sparks_fx_selected, momentary_fx_color, sanitize_sparks_fx_config,
    sparks_fx_param_default, sparks_fx_param_keys, sparks_fx_params, sparks_fx_params_map,
    sparks_fx_target_key, sparks_fx_type,
};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use visual_utils::{
    clip_display_line, scan_index_for_overlay, scan_section_count, scrolled_toast,
    touch_pan_pos_from_grid_x, trigger_gate_color, trigger_probability_allows, LedColor,
};

mod action_bindings;
mod action_control;
mod algorithm;
mod apply_payload;
mod apply_payload_instrument_values;
mod apply_payload_instruments;
mod apply_payload_layers;
mod apply_payload_mixer_values;
mod aux_auto_map;
mod aux_auto_map_fx_layouts;
mod aux_auto_map_instrument_layouts;
mod aux_auto_map_layouts;
mod aux_auto_map_overlay;
mod aux_binding_payload_apply;
mod aux_generated_behavior_turn;
mod behavior_menu;
mod behavior_menu_actions;
mod behavior_target_menu;
mod binding_payload;
mod binding_specs;
mod config;
mod construction;
mod construction_deferred;
mod construction_engine;
mod defaults;
mod deferred_flush;
mod device_input;
mod device_input_buttons;
mod factory_payload;
mod fx_bus_config;
mod fx_targets;
mod grid_assign;
mod grid_coords;
mod help_text;
mod instrument_collections;
mod instrument_runtime;
mod json_path;
mod layer_state;
mod led_color;
mod looper_config;
mod menu_apply;
mod menu_apply_fast;
mod menu_apply_fast_fx;
mod menu_apply_fast_fx_bus;
mod menu_apply_fast_runtime;
mod menu_apply_fast_values;
mod menu_apply_fx_state;
mod menu_apply_global;
mod menu_apply_instrument;
mod menu_apply_instrument_midi;
mod menu_apply_instrument_synth;
mod menu_apply_layers;
mod menu_apply_pulses_fx;
mod menu_apply_structural;
mod menu_value_apply;
mod modulation;
mod modulation_fx;
mod modulation_instrument;
mod modulation_instrument_numeric;
mod modulation_keys;
mod modulation_pulses;
mod modulation_sampler;
mod modulation_value;
mod outbox;
mod overlays;
mod overlays_fn;
mod pan_mapping;
mod pan_position;
mod payload_assign;
mod pulses_config;
mod pulses_payload;
mod pulses_payload_apply;
mod runner_config;
mod runtime_io;
mod sample_assignment_payload;
mod sample_browser;
mod sample_paths;
mod scan_overlay;
mod snapshot;
mod snapshot_audio_settings;
mod snapshot_display;
mod snapshot_leds;
mod snapshot_messages;
mod sparks_control;
mod sparks_fx_utils;
mod sparks_transpose;
mod sparks_trigger_gate;
mod state_instrument_types;
mod state_types;
mod store;
mod synth_config;
mod toast_state;
mod toast_text;
mod trigger_probability;
mod trigger_probability_payload;
mod velocity_curve;
mod visual_utils;

use crate::{clean_preset_name, fresh_preset_name};
pub use runner_config::NativeRunnerConfig;

use binding_payload::*;
use binding_specs::*;
use factory_payload::*;
use fx_bus_config::*;
use fx_targets::*;
use grid_coords::*;
use help_text::*;
use instrument_collections::*;
use instrument_runtime::*;
use json_path::*;
use menu_value_apply::*;
use modulation_instrument_numeric::*;
use outbox::NativeRunnerOutbox;
use pan_position::*;
use pulses_config::*;
use pulses_payload::*;
use sample_assignment_payload::*;
use sample_paths::*;
use sparks_trigger_gate::*;
use state_instrument_types::*;
use state_types::*;
use synth_config::*;
use trigger_probability_payload::*;
use velocity_curve::*;

const DEFAULT_ALGORITHM_STEP_PULSES: u32 = 12;
const OLED_BODY_ROWS: usize = 7;
#[cfg(not(test))]
const OLED_STARTUP_SPLASH_MS: u64 = 1_500;
#[cfg(test)]
const OLED_STARTUP_SPLASH_MS: u64 = 0;
const OLED_SLEEP_SPLASH_MS: u64 = 3_000;
const OLED_STARTUP_SPLASH_KEY: &str = "startup";
const OLED_SLEEP_SPLASH_KEY: &str = "sleep";
const OLED_SHUTDOWN_SPLASH_KEY: &str = "shutdown";
#[cfg(not(test))]
const DEFERRED_MENU_APPLY_MS: u64 = 24;
#[cfg(test)]
const DEFERRED_MENU_APPLY_MS: u64 = 24;

pub(super) fn normalize_voice_stealing_mode(value: &str) -> Option<&'static str> {
    match value {
        "none" | "off" => Some("none"),
        "fixed12" => Some("fixed12"),
        "fixed16" => Some("fixed16"),
        "auto-soft" | "lenient" => Some("auto-soft"),
        "auto-balanced" | "balanced" => Some("auto-balanced"),
        "auto-hard" | "aggressive" => Some("auto-hard"),
        _ => None,
    }
}

struct PendingMenuApply {
    due_at: Instant,
    key: String,
}

pub struct NativeRunner {
    engine: NativeLayerEngine,
    layer_engines: Vec<Option<NativeLayerEngine>>,
    behavior: NativeBehavior,
    behavior_config: Value,
    behavior_configs: BTreeMap<String, Value>,
    layer_behavior_configs: Vec<Value>,
    interpretation_profile: InterpretationProfile,
    mapping_config: platform_core::MappingConfig,
    base_mapping_config: platform_core::MappingConfig,
    global_sound: GlobalSoundConfig,
    note_behaviors: Vec<NoteBehavior>,
    current_ppqn_pulse: u64,
    swung_ppqn_pulse: u64,
    tick: u64,
    layer_ticks: Vec<u64>,
    algorithm_step_pulses: u32,
    algorithm_pulse_accumulator: u32,
    layer_algorithm_step_pulses: Vec<u32>,
    layer_pulse_accumulators: Vec<u32>,
    transport: RuntimeTransportState,
    sync_source: SyncSource,
    pending_resync: bool,
    bpm: f64,
    swing_pct: u8,
    audio_output_buffer_frames: u32,
    ui: NativeUiState,
    oled_mode: NativeOledMode,
    oled_splash_text: String,
    oled_splash_until: Option<Instant>,
    startup_splash_presented: bool,
    last_interaction_at: Instant,
    fn_hold_started_at: Option<Instant>,
    modifier_hint_started_at: Option<Instant>,
    midi_enabled: bool,
    preset_names: Vec<String>,
    current_preset_name: Option<String>,
    preset_draft_name: String,
    preset_rename_source: Option<String>,
    outbox: NativeRunnerOutbox,
    midi_outputs: Vec<MidiPort>,
    midi_inputs: Vec<MidiPort>,
    midi_status: Option<String>,
    selected_midi_output_id: Option<String>,
    selected_midi_input_id: Option<String>,
    input_events_while_paused: bool,
    voice_stealing_mode: String,
    midi_clock_out_enabled: bool,
    midi_clock_in_enabled: bool,
    midi_respond_to_start_stop: bool,
    sparks_mode: String,
    active_sparks_mode: String,
    sparks_fx_selected: Value,
    sparks_fx_assign: Option<Value>,
    sparks_fx_assignments: Vec<NativeSparksFxAssignment>,
    active_sparks_fx: Vec<(String, String)>,
    xy_touch: NativeXyTouch,
    xy_release: String,
    xy_invert_x: bool,
    xy_invert_y: bool,
    xy_x_binding: Option<NativeParamBinding>,
    xy_y_binding: Option<NativeParamBinding>,
    aux_auto_map_enabled: bool,
    param_mods: Vec<NativeParamMods>,
    trigger_gate_modes: Vec<String>,
    trigger_gate_restore_modes: Vec<Option<String>>,
    sparks_transpose_selected: Vec<bool>,
    sparks_transpose_enabled: Vec<bool>,
    sparks_transpose_offsets: Vec<i8>,
    sparks_transpose_active_notes: Vec<BTreeMap<(u8, u8), Vec<u8>>>,
    trigger_probability_assign: Option<usize>,
    trigger_probability_maps: Vec<Vec<String>>,
    layer_behavior_ids: Vec<String>,
    layer_names: Vec<String>,
    layer_auto_names: Vec<bool>,
    save_grid_states: Vec<bool>,
    pulses_layers: Vec<NativePulsesLayer>,
    aux_bindings: Vec<Option<NativeAuxBinding>>,
    active_layer_index: usize,
    instruments: Vec<NativeInstrumentSlot>,
    sample_assign: Option<(usize, usize)>,
    fx_buses: Vec<NativeFxBus>,
    global_fx_slots: Vec<String>,
    global_fx_params: Vec<Value>,
    sample_browser: Option<NativeSampleBrowser>,
    sample_builtin_favourite_dirs: Vec<String>,
    sample_favourite_dirs: Vec<String>,
    help_popup: Option<NativeHelpPopup>,
    confirm_dialog: Option<NativeConfirmDialog>,
    menu: NativeMenuModel,
    event_dot_on: bool,
    event_dot_pulses_remaining: u8,
    transport_flash: &'static str,
    transport_flash_pulses_remaining: u8,
    auto_save_default: bool,
    rolling_backups: bool,
    config_dirty: bool,
    pending_autosave_payload_due_at: Option<Instant>,
    last_backup_save_at: Option<Instant>,
    auto_save_flash_serial: u64,
    auto_save_flash_pulses_remaining: u8,
    audio_config_revision: u64,
    last_snapshot_audio_config_revision: Option<u64>,
    suppress_snapshot_response: bool,
    trigger_probability_rng: u64,
    toast: Option<NativeToast>,
    toast_expires_at: Option<Instant>,
    aux_turn_toast_cooldown_until: Option<Instant>,
    pending_aux_turn_toast: Option<PendingNativeToast>,
    pending_menu_apply: Option<PendingMenuApply>,
    pending_audio_output_buffer_reboot_prompt: bool,
    menu_scroll_offset: usize,
}

fn normalize_audio_output_buffer_frames(value: u32) -> u32 {
    match value {
        64 | 128 | 256 | 512 | 1024 | 2048 => value,
        _ => 256,
    }
}

#[cfg(test)]
mod tests;
