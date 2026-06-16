use crate::native_menu::{
    NativeAuxBindingConfig, NativeFxBusConfig, NativeMenuAction, NativeMenuConfig, NativeMenuModel,
    NativeParamBindingSpec, NativeParamModsConfig, NativeSensePartConfig, NativeValueLaneConfig,
};
#[cfg(test)]
use crate::protocol::{HostMessage, RunnerMessage};
#[cfg(test)]
use crate::protocol::RuntimeAudioCommand;
#[cfg(test)]
use crate::protocol::RuntimeStoreResult;
use crate::protocol::{
    MidiPort, RuntimeMomentaryFxTarget, RuntimePlatformEffect, RuntimeTransportState, SampleEntry,
    SyncSource,
};
use crate::runtime::{CoreRunner, RuntimeConfig};
use dance_fx_utils::{
    dance_fx_param_default, dance_fx_param_keys, dance_fx_params, dance_fx_params_map,
    dance_fx_target_key, dance_fx_type, default_dance_fx_selected, momentary_fx_color,
    sanitize_dance_fx_config,
};
use defaults::{
    default_fx_buses, default_global_fx_params, default_global_fx_slots, default_instruments,
    default_sense_parts, derive_bus_name, derive_instrument_name, fx_default_params,
    fx_slot_payload_with_params, note_unit_from_pulses, note_unit_to_pulses,
};
#[cfg(test)]
use modulation::{apply_sampler_assignments_for_instruments, sampler_assignment_velocity};
use modulation::{parse_instrument_binding_key, parse_part_behavior_config_binding_key};
#[cfg(test)]
use platform_core::CellTriggerIntent;
#[cfg(test)]
use platform_core::MusicalEvent;
use platform_core::{
    default_mapping_config, AxisStrategy, BehaviorActionInput, BehaviorConfigItem,
    BehaviorConfigItemType, DeviceInput, GlobalSoundConfig, GridInteraction,
    InterpretationEventProfile, InterpretationProfile, InterpretationStateProfile, NativeBehavior,
    NativePartEngine, NativePartEngineConfig, NoteBehavior, RangeMode, TickStrategy, TriggerAction,
    TriggerTarget, VelocityCurve, BUS_COUNT, GLOBAL_FX_SLOT_COUNT, GRID_HEIGHT, GRID_WIDTH,
    INSTRUMENT_COUNT, PAN_POSITION_COUNT, PART_COUNT, SAMPLE_SLOT_COUNT, TOUCH_FX_MAX_CONCURRENT,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use visual_utils::{
    add_dim_white_overlay, clip_display_line, dim_color, scan_index_for_overlay,
    scan_section_count, scrolled_toast, touch_pan_pos_from_grid_x, trigger_gate_color,
    trigger_probability_allows,
};

mod action_control;
mod algorithm;
mod apply_payload;
mod apply_payload_helpers;
mod apply_payload_instruments;
mod apply_payload_parts;
mod config;
mod dance_control;
mod dance_fx_utils;
mod defaults;
mod device_input;
mod menu_apply;
mod menu_apply_instrument;
mod menu_apply_sense_fx;
mod modulation;
mod overlays;
mod preset_names;
mod runtime_io;
mod sample_browser;
mod snapshot;
mod store;
mod visual_utils;

use preset_names::{clean_preset_name, fresh_preset_name};

const DEFAULT_ALGORITHM_STEP_PULSES: u32 = 12;
const OLED_BODY_ROWS: usize = 7;
const OLED_SLEEP_SPLASH_MS: u64 = 1_500;

#[derive(Clone, Debug, PartialEq, Eq)]
enum NativeOledMode {
    Normal,
    Splash,
    Off,
}

#[derive(Clone, Debug)]
pub struct NativeRunnerConfig {
    pub behavior_id: String,
    pub behavior_config: Value,
    pub interpretation_profile: InterpretationProfile,
    pub mapping_config: platform_core::MappingConfig,
    pub global_sound: GlobalSoundConfig,
    pub note_behaviors: Vec<NoteBehavior>,
    pub sync_source: SyncSource,
    pub bpm: f64,
}

impl Default for NativeRunnerConfig {
    fn default() -> Self {
        Self {
            behavior_id: "life".into(),
            behavior_config: Value::Null,
            interpretation_profile: InterpretationProfile {
                id: "native_profile".into(),
                event: InterpretationEventProfile { enabled: true },
                state: InterpretationStateProfile {
                    enabled: true,
                    tick: TickStrategy::WholeGridTransitions,
                },
                x: AxisStrategy::ScaleStep { step: 1 },
                y: AxisStrategy::TimingOnly,
            },
            mapping_config: default_mapping_config(),
            global_sound: GlobalSoundConfig {
                velocity_scale_pct: 100,
                velocity_curve: VelocityCurve::Linear,
                note_length_ms: 120,
            },
            note_behaviors: vec![NoteBehavior::Oneshot; 16],
            sync_source: SyncSource::Internal,
            bpm: 120.0,
        }
    }
}

#[derive(Clone, Debug)]
struct NativeUiState {
    display_brightness: u8,
    grid_brightness: u8,
    button_brightness: u8,
    master_volume: u8,
    ghost_cells: bool,
    numeric_display_mode: String,
    screen_sleep_seconds: u16,
    fn_held: bool,
    shift_held: bool,
    combined_modifier_held: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NativeInstrumentSlot {
    kind: String,
    note_behavior: String,
    auto_name: bool,
    name: String,
    volume: u8,
    pan_pos: u8,
    route: String,
    selected_sample_slot: usize,
    sample_paths: Vec<Option<String>>,
    sample_assignments: Vec<NativeSampleAssignment>,
    synth_config: Value,
    synth_gain_pct: u8,
    sample_tune_semis: i8,
    sample_gain_pct: u8,
    sample_amp_env: Value,
    sample_filter: Value,
    sample_filter_env: Value,
    sample_base_velocity: u8,
    sample_amp_velocity_sensitivity_pct: u8,
    sample_velocity_levels_enabled: bool,
    sample_velocity_high: u8,
    sample_velocity_medium: u8,
    sample_velocity_low: u8,
    midi_enabled: bool,
    midi_channel: u8,
    midi_velocity: u8,
    midi_duration_ms: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NativeSampleAssignment {
    x: usize,
    y: usize,
    sample_slot: usize,
    level: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NativeSampleBrowser {
    instrument_slot: usize,
    sample_slot: usize,
    dir: String,
    entries: Vec<SampleEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NativeSensePart {
    scan_mode: String,
    scan_axis: String,
    scan_unit: String,
    scan_direction: String,
    scan_sections: u8,
    scanned_slot: usize,
    scanned_action: String,
    scanned_empty_slot: usize,
    scanned_empty_action: String,
    event_enabled: bool,
    activate_slot: usize,
    activate_action: String,
    stable_slot: usize,
    stable_action: String,
    deactivate_slot: usize,
    deactivate_action: String,
    trigger_probability_mode: String,
    trigger_probability_low_pct: u8,
    trigger_probability_high_pct: u8,
    state_notes_enabled: bool,
    lowest_note: u8,
    highest_note: u8,
    starting_note: u8,
    scale: String,
    root: String,
    out_of_range: String,
    x_pitch_enabled: bool,
    x_pitch_steps: i32,
    x_pitch_restart_each_section: bool,
    y_pitch_enabled: bool,
    y_pitch_steps: i32,
    y_pitch_restart_each_section: bool,
    x_from: u8,
    x_to: u8,
    x_velocity: NativeValueLane,
    x_filter_cutoff: NativeValueLane,
    x_filter_resonance: NativeValueLane,
    y_from: u8,
    y_to: u8,
    y_velocity: NativeValueLane,
    y_filter_cutoff: NativeValueLane,
    y_filter_resonance: NativeValueLane,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NativeAuxBinding {
    turn_key: Option<String>,
    press_action: Option<NativeMenuAction>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NativeFxBus {
    name: String,
    slot1_type: String,
    slot1_params: Value,
    slot2_type: String,
    slot2_params: Value,
    pan_pos: u8,
    auto_name: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NativeHelpPopup {
    title: String,
    lines: Vec<String>,
    scroll: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NativeConfirmDialog {
    title: String,
    lines: Vec<String>,
    options: Vec<String>,
    cursor: usize,
    action: NativeMenuAction,
}

#[derive(Clone, Debug)]
struct NativeToast {
    message: String,
    offset: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct NativeXyTouch {
    x: f32,
    y: f32,
    active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NativeValueLane {
    enabled: bool,
    from: u8,
    to: u8,
    grid_offset: i32,
    curve: String,
}

#[derive(Clone, Debug, PartialEq)]
struct NativeParamBinding {
    key: String,
    label: Option<String>,
    kind: String,
    min: Option<f64>,
    max: Option<f64>,
    step: Option<f64>,
    options: Vec<String>,
    invert: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct NativeParamMods {
    x: Vec<Option<NativeParamBinding>>,
    y: Vec<Option<NativeParamBinding>>,
}

impl NativeInstrumentSlot {
    fn new(index: usize) -> Self {
        let kind = "synth".to_string();
        let synth_config = synth_preset_config("init");
        let sample_amp_env = synth_config
            .get("ampEnv")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let sample_filter = synth_config
            .get("filter")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let sample_filter_env = synth_config
            .get("filterEnv")
            .cloned()
            .unwrap_or_else(|| json!({}));
        Self {
            name: derive_instrument_name(index, &kind),
            kind,
            note_behavior: "oneshot".into(),
            auto_name: true,
            volume: 100,
            pan_pos: PAN_POSITION_COUNT / 2,
            route: "direct".into(),
            selected_sample_slot: 0,
            sample_paths: vec![None; SAMPLE_SLOT_COUNT],
            sample_assignments: Vec::new(),
            synth_config,
            synth_gain_pct: 80,
            sample_tune_semis: 0,
            sample_gain_pct: 100,
            sample_amp_env,
            sample_filter,
            sample_filter_env,
            sample_base_velocity: 100,
            sample_amp_velocity_sensitivity_pct: 100,
            sample_velocity_levels_enabled: false,
            sample_velocity_high: 120,
            sample_velocity_medium: 85,
            sample_velocity_low: 45,
            midi_enabled: false,
            midi_channel: 1,
            midi_velocity: 100,
            midi_duration_ms: 120,
        }
    }

    fn reset(index: usize) -> Self {
        let mut slot = Self::new(index);
        slot.kind = "none".into();
        slot.name = "none".into();
        slot.auto_name = true;
        slot.midi_enabled = false;
        slot.midi_channel = (index + 1).min(16) as u8;
        slot
    }
}

impl Default for NativeSensePart {
    fn default() -> Self {
        Self {
            scan_mode: "immediate".into(),
            scan_axis: "columns".into(),
            scan_unit: "1/16".into(),
            scan_direction: "forward".into(),
            scan_sections: 1,
            scanned_slot: 0,
            scanned_action: "note_on".into(),
            scanned_empty_slot: usize::MAX,
            scanned_empty_action: "none".into(),
            event_enabled: true,
            activate_slot: 0,
            activate_action: "note_on".into(),
            stable_slot: 0,
            stable_action: "none".into(),
            deactivate_slot: 0,
            deactivate_action: "note_off".into(),
            trigger_probability_mode: "full".into(),
            trigger_probability_low_pct: 25,
            trigger_probability_high_pct: 75,
            state_notes_enabled: true,
            lowest_note: 36,
            highest_note: 74,
            starting_note: 60,
            scale: "major_pentatonic".into(),
            root: "D".into(),
            out_of_range: "clamp".into(),
            x_pitch_enabled: true,
            x_pitch_steps: 0,
            x_pitch_restart_each_section: false,
            y_pitch_enabled: true,
            y_pitch_steps: 1,
            y_pitch_restart_each_section: false,
            x_from: 0,
            x_to: 7,
            x_velocity: NativeValueLane::velocity_default(),
            x_filter_cutoff: NativeValueLane::filter_cutoff_default(),
            x_filter_resonance: NativeValueLane::filter_resonance_default(),
            y_from: 0,
            y_to: 7,
            y_velocity: NativeValueLane::velocity_default(),
            y_filter_cutoff: NativeValueLane::filter_cutoff_default(),
            y_filter_resonance: NativeValueLane::filter_resonance_default(),
        }
    }
}

impl NativeValueLane {
    fn velocity_default() -> Self {
        Self {
            enabled: false,
            from: 1,
            to: 127,
            grid_offset: 0,
            curve: "linear".into(),
        }
    }

    fn filter_cutoff_default() -> Self {
        Self {
            enabled: false,
            from: 20,
            to: 127,
            grid_offset: 0,
            curve: "linear".into(),
        }
    }

    fn filter_resonance_default() -> Self {
        Self {
            enabled: false,
            from: 10,
            to: 90,
            grid_offset: 0,
            curve: "linear".into(),
        }
    }
}

impl Default for NativeParamMods {
    fn default() -> Self {
        Self {
            x: vec![None, None],
            y: vec![None, None],
        }
    }
}

impl Default for NativeFxBus {
    fn default() -> Self {
        Self {
            name: "(none)".into(),
            slot1_type: "none".into(),
            slot1_params: json!({}),
            slot2_type: "none".into(),
            slot2_params: json!({}),
            pan_pos: 16,
            auto_name: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct NativeDanceFxAssignment {
    x: usize,
    y: usize,
    config: Value,
}

impl Default for NativeUiState {
    fn default() -> Self {
        Self {
            display_brightness: 75,
            grid_brightness: 75,
            button_brightness: 75,
            master_volume: 73,
            ghost_cells: false,
            numeric_display_mode: "bar+numbers".into(),
            screen_sleep_seconds: 60,
            fn_held: false,
            shift_held: false,
            combined_modifier_held: false,
        }
    }
}

pub struct NativeRunner {
    engine: NativePartEngine,
    part_engines: Vec<Option<NativePartEngine>>,
    behavior: NativeBehavior,
    behavior_config: Value,
    behavior_configs: BTreeMap<String, Value>,
    part_behavior_configs: Vec<Value>,
    interpretation_profile: InterpretationProfile,
    mapping_config: platform_core::MappingConfig,
    global_sound: GlobalSoundConfig,
    note_behaviors: Vec<NoteBehavior>,
    current_ppqn_pulse: u64,
    tick: u64,
    algorithm_step_pulses: u32,
    algorithm_pulse_accumulator: u32,
    part_algorithm_step_pulses: Vec<u32>,
    part_pulse_accumulators: Vec<u32>,
    transport: RuntimeTransportState,
    sync_source: SyncSource,
    pending_resync: bool,
    bpm: f64,
    ui: NativeUiState,
    oled_mode: NativeOledMode,
    oled_splash_text: String,
    oled_splash_until: Option<Instant>,
    last_interaction_at: Instant,
    midi_enabled: bool,
    preset_names: Vec<String>,
    current_preset_name: Option<String>,
    preset_draft_name: String,
    preset_rename_source: Option<String>,
    queued_platform_effects: Vec<RuntimePlatformEffect>,
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
    dance_mode: String,
    active_dance_mode: String,
    dance_fx_selected: Value,
    dance_fx_assign: Option<Value>,
    dance_fx_assignments: Vec<NativeDanceFxAssignment>,
    active_dance_fx: Vec<(String, String)>,
    xy_touch: NativeXyTouch,
    xy_release: String,
    xy_invert_x: bool,
    xy_invert_y: bool,
    xy_x_binding: Option<NativeParamBinding>,
    xy_y_binding: Option<NativeParamBinding>,
    param_mods: Vec<NativeParamMods>,
    trigger_gate_modes: Vec<String>,
    trigger_gate_restore_modes: Vec<Option<String>>,
    trigger_probability_assign: Option<usize>,
    trigger_probability_maps: Vec<Vec<String>>,
    part_behavior_ids: Vec<String>,
    part_names: Vec<String>,
    part_auto_names: Vec<bool>,
    save_grid_states: Vec<bool>,
    sense_parts: Vec<NativeSensePart>,
    aux_bindings: Vec<Option<NativeAuxBinding>>,
    active_part_index: usize,
    instruments: Vec<NativeInstrumentSlot>,
    sample_assign: Option<(usize, usize)>,
    fx_buses: Vec<NativeFxBus>,
    global_fx_slots: Vec<String>,
    global_fx_params: Vec<Value>,
    sample_browser: Option<NativeSampleBrowser>,
    help_popup: Option<NativeHelpPopup>,
    confirm_dialog: Option<NativeConfirmDialog>,
    menu: NativeMenuModel,
    event_dot_on: bool,
    event_dot_pulses_remaining: u8,
    transport_flash: &'static str,
    transport_flash_pulses_remaining: u8,
    auto_save_default: bool,
    config_dirty: bool,
    auto_save_flash_serial: u64,
    auto_save_flash_pulses_remaining: u8,
    trigger_probability_rng: u64,
    toast: Option<NativeToast>,
}

impl NativeRunner {
    pub fn new(config: NativeRunnerConfig) -> Result<Self, String> {
        let behavior = platform_core::get_native_behavior(&config.behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{}`", config.behavior_id))?;
        let engine = Self::build_engine(
            behavior,
            config.behavior_config.clone(),
            config.interpretation_profile.clone(),
            config.mapping_config.clone(),
            config.global_sound.clone(),
            config.note_behaviors.clone(),
            0,
        )?;
        let ui = NativeUiState::default();
        let now = Instant::now();
        let instruments = default_instruments();
        let sense_parts = default_sense_parts();
        let fx_buses = default_fx_buses();
        let global_fx_slots = default_global_fx_slots();
        let global_fx_params = default_global_fx_params();
        let menu = NativeMenuModel::new(NativeMenuConfig {
            behavior_id: behavior.id().into(),
            behavior_ids: platform_core::list_native_behavior_ids()
                .iter()
                .map(|id| (*id).to_string())
                .collect(),
            l1_items: vec![],
            part_labels: (0..PART_COUNT)
                .map(|index| format!("P{}: life", index + 1))
                .collect(),
            part_names: vec![behavior.id().into(); PART_COUNT],
            part_auto_names: vec![true; PART_COUNT],
            sense_parts: sense_part_configs(&sense_parts),
            active_part_index: 0,
            param_mods: vec![NativeParamModsConfig::default(); PART_COUNT],
            xy_x_binding: None,
            xy_y_binding: None,
            aux_bindings: vec![NativeAuxBindingConfig::default(); 4],
            instrument_labels: instrument_labels(&instruments),
            instrument_names: instrument_names(&instruments),
            instrument_types: instrument_types(&instruments),
            instrument_auto_names: instrument_auto_names(&instruments),
            instrument_note_behaviors: instrument_note_behaviors(&instruments),
            instrument_routes: instrument_routes(&instruments),
            instrument_volumes: instrument_volumes(&instruments),
            instrument_pan_positions: instrument_pan_positions(&instruments),
            instrument_sample_slots: instrument_sample_slots(&instruments),
            instrument_synth_configs: instrument_synth_configs(&instruments),
            instrument_synth_osc1_waveforms: instrument_synth_osc1_waveforms(&instruments),
            instrument_synth_osc2_waveforms: instrument_synth_osc2_waveforms(&instruments),
            instrument_synth_filter_types: instrument_synth_filter_types(&instruments),
            instrument_synth_filter_cutoffs: instrument_synth_filter_cutoffs(&instruments),
            instrument_synth_gain_pct: instrument_synth_gain_pct(&instruments),
            instrument_synth_filter_resonance: instrument_synth_filter_resonance(&instruments),
            instrument_sample_tune_semis: instrument_sample_tune_semis(&instruments),
            instrument_sample_gain_pct: instrument_sample_gain_pct(&instruments),
            instrument_sample_base_velocity: instrument_sample_base_velocity(&instruments),
            instrument_sample_amp_velocity_sensitivity_pct:
                instrument_sample_amp_velocity_sensitivity_pct(&instruments),
            instrument_sample_velocity_levels_enabled: instrument_sample_velocity_levels_enabled(
                &instruments,
            ),
            instrument_sample_velocity_high: instrument_sample_velocity_high(&instruments),
            instrument_sample_velocity_medium: instrument_sample_velocity_medium(&instruments),
            instrument_sample_velocity_low: instrument_sample_velocity_low(&instruments),
            instrument_sample_amp_envs: instrument_sample_amp_envs(&instruments),
            instrument_sample_filters: instrument_sample_filters(&instruments),
            instrument_sample_filter_envs: instrument_sample_filter_envs(&instruments),
            instrument_midi_enabled: instrument_midi_enabled(&instruments),
            instrument_midi_channels: instrument_midi_channels(&instruments),
            instrument_midi_velocity: instrument_midi_velocity(&instruments),
            instrument_midi_duration_ms: instrument_midi_duration_ms(&instruments),
            fx_buses: fx_bus_configs(&fx_buses),
            global_fx_slots: global_fx_slots.clone(),
            global_fx_params: global_fx_params.clone(),
            sample_browser: None,
            algorithm_step_pulses: DEFAULT_ALGORITHM_STEP_PULSES,
            master_volume: ui.master_volume,
            note_length_ms: config.global_sound.note_length_ms as u16,
            velocity_scale_pct: config.global_sound.velocity_scale_pct,
            velocity_curve: velocity_curve_id(config.global_sound.velocity_curve).into(),
            voice_stealing_mode: "balanced".into(),
            auto_save_default: false,
            ghost_cells: ui.ghost_cells,
            input_events_while_paused: true,
            numeric_display_mode: ui.numeric_display_mode.clone(),
            screen_sleep_seconds: ui.screen_sleep_seconds,
            grid_brightness: ui.grid_brightness,
            display_brightness: ui.display_brightness,
            button_brightness: ui.button_brightness,
            midi_enabled: false,
            midi_clock_out_enabled: false,
            midi_clock_in_enabled: false,
            midi_respond_to_start_stop: true,
            preset_names: Vec::new(),
            preset_draft_name: fresh_preset_name(),
            preset_rename_source: None,
            midi_outputs: Vec::new(),
            midi_inputs: Vec::new(),
            dance_mode: "none".into(),
            dance_fx_type: "none".into(),
            dance_fx_target: "master".into(),
            dance_fx_params: serde_json::Map::new(),
            xy_release: "sample-hold".into(),
            xy_invert_x: false,
            xy_invert_y: false,
            bpm: config.bpm.round().clamp(20.0, 300.0) as u16,
            sync_source: config.sync_source.clone(),
        });
        let mut part_engines = Vec::new();
        part_engines.resize_with(PART_COUNT, || None);
        for (index, slot) in part_engines.iter_mut().enumerate().skip(1) {
            let part_behavior = platform_core::get_native_behavior(config.behavior_id.as_str())
                .ok_or_else(|| format!("unsupported native behavior `{}`", config.behavior_id))?;
            *slot = Some(Self::build_engine(
                part_behavior,
                config.behavior_config.clone(),
                config.interpretation_profile.clone(),
                config.mapping_config.clone(),
                config.global_sound.clone(),
                config.note_behaviors.clone(),
                index,
            )?);
        }
        let mut runner = Self {
            engine,
            part_engines,
            behavior,
            behavior_config: config.behavior_config.clone(),
            behavior_configs: BTreeMap::from([(
                behavior.id().to_string(),
                config.behavior_config.clone(),
            )]),
            part_behavior_configs: vec![config.behavior_config; PART_COUNT],
            interpretation_profile: config.interpretation_profile,
            mapping_config: config.mapping_config,
            global_sound: config.global_sound,
            note_behaviors: config.note_behaviors,
            current_ppqn_pulse: 0,
            tick: 0,
            algorithm_step_pulses: DEFAULT_ALGORITHM_STEP_PULSES,
            algorithm_pulse_accumulator: 0,
            part_algorithm_step_pulses: vec![DEFAULT_ALGORITHM_STEP_PULSES; PART_COUNT],
            part_pulse_accumulators: vec![0; PART_COUNT],
            transport: RuntimeTransportState::Stopped,
            sync_source: config.sync_source,
            pending_resync: false,
            bpm: config.bpm,
            ui,
            oled_mode: NativeOledMode::Normal,
            oled_splash_text: String::new(),
            oled_splash_until: None,
            last_interaction_at: now,
            midi_enabled: false,
            preset_names: Vec::new(),
            current_preset_name: None,
            preset_draft_name: fresh_preset_name(),
            preset_rename_source: None,
            queued_platform_effects: Vec::new(),
            midi_outputs: Vec::new(),
            midi_inputs: Vec::new(),
            midi_status: None,
            selected_midi_output_id: None,
            selected_midi_input_id: None,
            input_events_while_paused: true,
            voice_stealing_mode: "balanced".into(),
            midi_clock_out_enabled: false,
            midi_clock_in_enabled: false,
            midi_respond_to_start_stop: true,
            dance_mode: "none".into(),
            active_dance_mode: "none".into(),
            dance_fx_selected: default_dance_fx_selected(),
            dance_fx_assign: None,
            dance_fx_assignments: vec![],
            active_dance_fx: Vec::new(),
            xy_touch: NativeXyTouch {
                x: 0.5,
                y: 0.5,
                active: false,
            },
            xy_release: "sample-hold".into(),
            xy_invert_x: false,
            xy_invert_y: false,
            xy_x_binding: None,
            xy_y_binding: None,
            param_mods: vec![NativeParamMods::default(); PART_COUNT],
            trigger_gate_modes: vec!["full".into(); PART_COUNT],
            trigger_gate_restore_modes: vec![None; PART_COUNT],
            trigger_probability_assign: None,
            trigger_probability_maps: vec![
                vec!["full".into(); GRID_WIDTH * GRID_HEIGHT];
                PART_COUNT
            ],
            part_behavior_ids: vec![behavior.id().into(); PART_COUNT],
            part_names: vec![behavior.id().into(); PART_COUNT],
            part_auto_names: vec![true; PART_COUNT],
            save_grid_states: vec![true; PART_COUNT],
            sense_parts,
            aux_bindings: vec![None; 4],
            active_part_index: 0,
            instruments,
            sample_assign: None,
            fx_buses,
            global_fx_slots,
            global_fx_params,
            sample_browser: None,
            help_popup: None,
            confirm_dialog: None,
            menu,
            event_dot_on: false,
            event_dot_pulses_remaining: 0,
            transport_flash: "none",
            transport_flash_pulses_remaining: 0,
            auto_save_default: false,
            config_dirty: false,
            auto_save_flash_serial: 0,
            auto_save_flash_pulses_remaining: 0,
            trigger_probability_rng: 0xC311_5A7E_2024_0001,
            toast: None,
        };
        runner.seed_visible_state()?;
        runner.refresh_active_mapping_config();
        runner.refresh_active_interpretation_profile();
        runner
            .engine
            .set_interpretation_profile(runner.interpretation_profile.clone());
        runner.menu.rebuild(runner.menu_config());
        Ok(runner)
    }

    pub fn apply_runtime_config(&mut self, config: &RuntimeConfig) {
        self.sync_source = config.sync_source.clone();
        self.bpm = config.bpm;
        self.menu.rebuild(self.menu_config());
    }

    #[cfg(test)]
    fn set_toast_for_test(&mut self, message: &str) {
        self.toast = Some(NativeToast {
            message: message.into(),
            offset: 0,
        });
    }

    #[cfg(test)]
    fn advance_toast_for_test(&mut self) {
        if let Some(toast) = &mut self.toast {
            toast.offset = toast.offset.saturating_add(1);
        }
    }

    fn build_engine(
        behavior: NativeBehavior,
        behavior_config: Value,
        interpretation_profile: InterpretationProfile,
        mapping_config: platform_core::MappingConfig,
        global_sound: GlobalSoundConfig,
        note_behaviors: Vec<NoteBehavior>,
        part_index: usize,
    ) -> Result<NativePartEngine, String> {
        NativePartEngine::new(NativePartEngineConfig {
            behavior,
            behavior_config,
            interpretation_profile,
            mapping_config,
            global_sound,
            note_behaviors,
            part_index,
        })
    }

    fn rebuild_engine(&mut self, behavior: NativeBehavior) -> Result<(), String> {
        self.engine = Self::build_engine(
            behavior,
            self.behavior_config.clone(),
            self.interpretation_profile.clone(),
            self.mapping_config.clone(),
            self.global_sound.clone(),
            self.note_behaviors.clone(),
            self.active_part_index,
        )?;
        self.behavior = behavior;
        self.tick = 0;
        self.current_ppqn_pulse = 0;
        self.algorithm_pulse_accumulator = 0;
        self.transport_flash = "none";
        self.transport_flash_pulses_remaining = 0;
        self.event_dot_on = false;
        self.event_dot_pulses_remaining = 0;
        self.menu.rebuild(self.menu_config());
        Ok(())
    }

    fn sync_engine_runtime_config(&mut self) {
        self.note_behaviors = note_behaviors_from_instruments(&self.instruments);
        self.engine.set_global_sound(self.global_sound.clone());
        self.engine.set_note_behaviors(self.note_behaviors.clone());
        for engine in self.part_engines.iter_mut().flatten() {
            engine.set_global_sound(self.global_sound.clone());
            engine.set_note_behaviors(self.note_behaviors.clone());
        }
    }

    fn record_display_interaction(&mut self) {
        self.last_interaction_at = Instant::now();
        if self.oled_mode != NativeOledMode::Normal {
            self.oled_mode = NativeOledMode::Normal;
            self.oled_splash_text.clear();
            self.oled_splash_until = None;
        }
    }

    fn advance_oled_sleep_state(&mut self) {
        let now = Instant::now();
        if self.ui.screen_sleep_seconds == 0 {
            if self.oled_mode != NativeOledMode::Normal {
                self.oled_mode = NativeOledMode::Normal;
                self.oled_splash_text.clear();
                self.oled_splash_until = None;
            }
            return;
        }
        if self.oled_mode == NativeOledMode::Normal
            && now.duration_since(self.last_interaction_at)
                >= Duration::from_secs(u64::from(self.ui.screen_sleep_seconds))
        {
            self.oled_mode = NativeOledMode::Splash;
            self.oled_splash_text = "Going to sleep".into();
            self.oled_splash_until = Some(now + Duration::from_millis(OLED_SLEEP_SPLASH_MS));
            return;
        }
        if self.oled_mode == NativeOledMode::Splash
            && self
                .oled_splash_until
                .is_some_and(|deadline| now >= deadline)
        {
            self.oled_mode = NativeOledMode::Off;
            self.oled_splash_until = None;
        }
    }

    fn mapping_config_for_part(&self, part_index: usize) -> platform_core::MappingConfig {
        let Some(sense) = self.sense_parts.get(part_index) else {
            return self.mapping_config.clone();
        };
        let mut mapping = self.mapping_config.clone();
        mapping.base_midi_note = i32::from(sense.lowest_note.min(sense.highest_note));
        mapping.starting_midi_note = i32::from(sense.starting_note);
        mapping.max_midi_note = i32::from(sense.lowest_note.max(sense.highest_note));
        mapping.column_step_degrees = if sense.x_pitch_enabled {
            sense.x_pitch_steps
        } else {
            0
        };
        mapping.row_step_degrees = if sense.y_pitch_enabled {
            sense.y_pitch_steps
        } else {
            0
        };
        mapping.range_mode = if sense.out_of_range == "clamp" {
            RangeMode::Clamp
        } else {
            RangeMode::Wrap
        };
        mapping.scale = scale_steps(&sense.scale, &sense.root);
        mapping.activate = trigger_target(sense.activate_slot, &sense.activate_action, 96, 150);
        mapping.stable = trigger_target(sense.stable_slot, &sense.stable_action, 88, 130);
        mapping.deactivate =
            trigger_target(sense.deactivate_slot, &sense.deactivate_action, 68, 90);
        mapping.scanned = trigger_target(sense.scanned_slot, &sense.scanned_action, 88, 130);
        mapping.scanned_empty = trigger_target(
            sense.scanned_empty_slot,
            &sense.scanned_empty_action,
            68,
            90,
        );
        mapping
    }

    fn interpretation_profile_for_part(&self, part_index: usize) -> InterpretationProfile {
        let Some(sense) = self.sense_parts.get(part_index) else {
            return self.interpretation_profile.clone();
        };
        let mut profile = self.interpretation_profile.clone();
        profile.event.enabled = sense.event_enabled;
        profile.state.enabled = sense.state_notes_enabled;
        let sections = if sense.scan_sections <= 1 {
            None
        } else {
            Some(usize::from(sense.scan_sections))
        };
        profile.state.tick = if sense.scan_mode == "scanning" {
            let reverse = sense.scan_direction == "reverse";
            if sense.scan_axis == "columns" {
                TickStrategy::ScanColumnActive { sections, reverse }
            } else {
                TickStrategy::ScanRowActive { sections, reverse }
            }
        } else {
            TickStrategy::WholeGridTransitions
        };
        profile.x = AxisStrategy::ScaleStep {
            step: if sense.x_pitch_enabled {
                sense.x_pitch_steps.max(0) as usize
            } else {
                0
            },
        };
        profile.y = AxisStrategy::ScaleStep {
            step: if sense.y_pitch_enabled {
                sense.y_pitch_steps.max(0) as usize
            } else {
                0
            },
        };
        profile
    }

    fn store_active_engine(&mut self) {
        if let Some(config) = self.part_behavior_configs.get_mut(self.active_part_index) {
            *config = self.behavior_config.clone();
        }
        if let Some(slot) = self.part_engines.get_mut(self.active_part_index) {
            let placeholder = Self::build_engine(
                self.behavior,
                self.behavior_config.clone(),
                self.interpretation_profile.clone(),
                self.mapping_config.clone(),
                self.global_sound.clone(),
                self.note_behaviors.clone(),
                self.active_part_index,
            )
            .expect("active engine placeholder");
            *slot = Some(std::mem::replace(&mut self.engine, placeholder));
        }
    }

    fn activate_engine(&mut self, index: usize) -> Result<(), String> {
        let behavior_id = self
            .part_behavior_ids
            .get(index)
            .cloned()
            .unwrap_or_else(|| self.behavior.id().into());
        let behavior = platform_core::get_native_behavior(&behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{behavior_id}`"))?;
        let profile = self.interpretation_profile_for_part(index);
        let mapping = self.mapping_config_for_part(index);
        let next = if let Some(slot) = self.part_engines.get_mut(index) {
            slot.take()
        } else {
            None
        };
        self.engine = if let Some(mut engine) = next {
            engine.set_interpretation_profile(profile.clone());
            engine.set_mapping_config(mapping.clone());
            engine
        } else {
            Self::build_engine(
                behavior,
                self.part_behavior_configs
                    .get(index)
                    .filter(|config| !config.is_null())
                    .cloned()
                    .or_else(|| self.behavior_configs.get(&behavior_id).cloned())
                    .unwrap_or(Value::Null),
                profile.clone(),
                mapping.clone(),
                self.global_sound.clone(),
                self.note_behaviors.clone(),
                index,
            )?
        };
        self.behavior = behavior;
        self.behavior_config = self
            .part_behavior_configs
            .get(index)
            .filter(|config| !config.is_null())
            .cloned()
            .or_else(|| self.behavior_configs.get(&behavior_id).cloned())
            .unwrap_or(Value::Null);
        self.interpretation_profile = profile;
        self.mapping_config = mapping;
        Ok(())
    }

    fn serialized_state_for_part(&self, index: usize) -> Result<Value, String> {
        if index == self.active_part_index {
            return self.engine.serialized_state();
        }
        if let Some(Some(engine)) = self.part_engines.get(index) {
            return engine.serialized_state();
        }
        Ok(Value::Null)
    }

    fn l1_payload_for_part(&self, index: usize, behavior_id: &str) -> Value {
        let step_pulses = if index == self.active_part_index {
            self.algorithm_step_pulses
        } else {
            self.part_algorithm_step_pulses
                .get(index)
                .copied()
                .unwrap_or(DEFAULT_ALGORITHM_STEP_PULSES)
        };
        let save_grid_state = self.save_grid_states.get(index).copied().unwrap_or(true);
        let mut l1 = serde_json::Map::new();
        l1.insert("behaviorId".into(), json!(behavior_id));
        l1.insert("stepRate".into(), json!(note_unit_from_pulses(step_pulses)));
        l1.insert(
            "behaviorConfig".into(),
            if index == self.active_part_index {
                self.behavior_config.clone()
            } else {
                self.part_behavior_configs
                    .get(index)
                    .cloned()
                    .unwrap_or(Value::Null)
            },
        );
        l1.insert("saveGridState".into(), json!(save_grid_state));
        if save_grid_state {
            if let Ok(state) = self.serialized_state_for_part(index) {
                if !state.is_null() {
                    l1.insert("savedState".into(), state);
                }
            }
        }
        Value::Object(l1)
    }

    fn rebuild_part_engine_from_payload(
        &self,
        index: usize,
        behavior_id: &str,
        l1: &Value,
    ) -> Result<NativePartEngine, String> {
        let behavior = platform_core::get_native_behavior(behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{behavior_id}`"))?;
        let config = NativePartEngineConfig {
            behavior,
            behavior_config: l1.get("behaviorConfig").cloned().unwrap_or(Value::Null),
            interpretation_profile: self.interpretation_profile_for_part(index),
            mapping_config: self.mapping_config_for_part(index),
            global_sound: self.global_sound.clone(),
            note_behaviors: self.note_behaviors.clone(),
            part_index: index,
        };
        let save_grid_state = l1
            .get("saveGridState")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let state = l1
            .get("savedState")
            .filter(|value| !value.is_null())
            .or_else(|| l1.get("behaviorState").filter(|value| !value.is_null()));
        if let Some(state) = state.filter(|_| save_grid_state) {
            NativePartEngine::from_serialized_state(config, state.clone())
        } else {
            NativePartEngine::new(config)
        }
    }

    fn remap_bindings_for_behavior_change(
        &mut self,
        from_behavior_id: &str,
        to_behavior_id: &str,
        part_index: usize,
    ) {
        let Some(to_behavior) = platform_core::get_native_behavior(to_behavior_id) else {
            return;
        };
        if let Some(param_mods) = self.param_mods.get_mut(part_index) {
            for binding in param_mods.x.iter_mut().chain(param_mods.y.iter_mut()) {
                if let Some(current) = binding.clone() {
                    if let Some(next) =
                        remap_behavior_param_binding(current, to_behavior, part_index)
                    {
                        *binding = Some(next);
                    }
                }
            }
        }

        let from_action =
            platform_core::get_native_behavior(from_behavior_id).and_then(primary_behavior_action);
        let to_action = primary_behavior_action(to_behavior);
        for binding in &mut self.aux_bindings {
            let Some(aux) = binding else {
                continue;
            };
            if let Some(turn_key) = aux.turn_key.clone() {
                if let Some(remapped) = remap_behavior_binding_key(&turn_key, to_behavior, None) {
                    aux.turn_key = Some(remapped.key);
                }
            }
            if let (Some((from_action, _)), Some(NativeMenuAction::BehaviorAction(action))) =
                (&from_action, aux.press_action.as_ref())
            {
                if action == from_action {
                    aux.press_action = to_action
                        .as_ref()
                        .map(|(action, _)| NativeMenuAction::BehaviorAction(action.clone()));
                }
            }
            if aux.turn_key.is_none() && aux.press_action.is_none() {
                *binding = None;
            }
        }
    }

    fn l1_menu_items(&self) -> Vec<crate::native_menu::NativeMenuItem> {
        let mut items = vec![
            crate::native_menu::NativeMenuItem {
                label: "Behavior".into(),
                key: Some("behaviorId".into()),
                value: crate::native_menu::NativeMenuValue::Enum {
                    options: platform_core::list_native_behavior_ids()
                        .iter()
                        .map(|id| (*id).to_string())
                        .collect(),
                    selected: platform_core::list_native_behavior_ids()
                        .iter()
                        .position(|id| *id == self.behavior.id())
                        .unwrap_or(0),
                },
                children: vec![],
            },
            crate::native_menu::NativeMenuItem {
                label: "Auto Name".into(),
                key: Some(format!("parts.{}.autoName", self.active_part_index)),
                value: crate::native_menu::NativeMenuValue::Bool {
                    value: self
                        .part_auto_names
                        .get(self.active_part_index)
                        .copied()
                        .unwrap_or(true),
                },
                children: vec![],
            },
            crate::native_menu::NativeMenuItem {
                label: "Part Name".into(),
                key: Some(format!("parts.{}.name", self.active_part_index)),
                value: crate::native_menu::NativeMenuValue::Text {
                    value: self
                        .part_names
                        .get(self.active_part_index)
                        .cloned()
                        .unwrap_or_else(|| self.behavior.id().into()),
                    max_len: 32,
                    cursor: 0,
                },
                children: vec![],
            },
            crate::native_menu::NativeMenuItem {
                label: "Step Rate".into(),
                key: Some("algorithmStep".into()),
                value: crate::native_menu::NativeMenuValue::Enum {
                    options: vec!["1/16", "1/8", "1/4", "1/2", "1/1"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                    selected: [6, 12, 24, 48, 96]
                        .iter()
                        .position(|value| *value == self.algorithm_step_pulses)
                        .unwrap_or(1),
                },
                children: vec![],
            },
        ];

        if let Ok(Some(config_items)) = self.behavior.config_menu(&self.engine_state()) {
            for item in config_items {
                if let Some(menu_item) = self.behavior_menu_item(item) {
                    items.push(menu_item);
                }
            }
        }

        items.push(crate::native_menu::NativeMenuItem {
            label: "Reset".into(),
            key: Some("behavior.reset".into()),
            value: crate::native_menu::NativeMenuValue::Action(NativeMenuAction::ResetBehavior),
            children: vec![],
        });
        items
    }

    fn part_labels(&self) -> Vec<String> {
        self.part_names
            .iter()
            .enumerate()
            .map(|(index, name)| format!("P{}: {}", index + 1, name))
            .collect()
    }

    fn engine_state(&self) -> platform_core::NativeBehaviorState {
        self.engine.state().clone()
    }

    fn behavior_menu_item(
        &self,
        item: BehaviorConfigItem,
    ) -> Option<crate::native_menu::NativeMenuItem> {
        let key = format!(
            "parts.{}.l1.behaviorConfig.{}",
            self.active_part_index, item.key
        );
        match item.item_type {
            BehaviorConfigItemType::Number => Some(crate::native_menu::NativeMenuItem {
                label: item.label,
                key: Some(key.clone()),
                value: crate::native_menu::NativeMenuValue::Number {
                    value: self
                        .behavior_config_number(&item.key)
                        .unwrap_or(item.min.unwrap_or(0)),
                    min: item.min.unwrap_or(0),
                    max: item.max.unwrap_or(127),
                    step: item.step.unwrap_or(1),
                },
                children: vec![],
            }),
            BehaviorConfigItemType::Action => Some(crate::native_menu::NativeMenuItem {
                label: item.label,
                key: Some(key),
                value: crate::native_menu::NativeMenuValue::Action(
                    NativeMenuAction::BehaviorAction(item.key),
                ),
                children: vec![],
            }),
            BehaviorConfigItemType::Bool => Some(crate::native_menu::NativeMenuItem {
                label: item.label,
                key: Some(key),
                value: crate::native_menu::NativeMenuValue::Bool {
                    value: self
                        .behavior_config
                        .get(&item.key)
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                },
                children: vec![],
            }),
            BehaviorConfigItemType::Enum => {
                let options = item.options.unwrap_or_default();
                let selected_value = self
                    .behavior_config
                    .get(&item.key)
                    .and_then(Value::as_str)
                    .unwrap_or_else(|| options.first().map(String::as_str).unwrap_or(""));
                let selected = options
                    .iter()
                    .position(|option| option == selected_value)
                    .unwrap_or(0);
                Some(crate::native_menu::NativeMenuItem {
                    label: item.label,
                    key: Some(key),
                    value: crate::native_menu::NativeMenuValue::Enum { options, selected },
                    children: vec![],
                })
            }
        }
    }

    fn behavior_config_number(&self, key: &str) -> Option<i32> {
        self.behavior_config
            .get(key)
            .and_then(|value| value.as_i64())
            .map(|value| value as i32)
    }

    fn behavior_config_from_menu(&self) -> Result<Value, String> {
        let mut object = self
            .behavior_config
            .as_object()
            .cloned()
            .unwrap_or_default();

        if let Ok(Some(config_items)) = self.behavior.config_menu(&self.engine_state()) {
            for item in config_items {
                let key = format!(
                    "parts.{}.l1.behaviorConfig.{}",
                    self.active_part_index, item.key
                );
                match item.item_type {
                    BehaviorConfigItemType::Number => {
                        if let Some(value) = self.menu.number_for_key(&key) {
                            object.insert(item.key, Value::from(value));
                        }
                    }
                    BehaviorConfigItemType::Bool => {
                        if let Some(value) = self.menu.value_for_key(&key) {
                            object.insert(item.key, Value::from(value == "true"));
                        }
                    }
                    BehaviorConfigItemType::Enum => {
                        if let Some(value) = self.menu.value_for_key(&key) {
                            object.insert(item.key, Value::from(value));
                        }
                    }
                    BehaviorConfigItemType::Action => {}
                }
            }
        }

        Ok(Value::Object(object))
    }

    fn trigger_behavior_action(&mut self, action_type: String) -> Result<(), String> {
        self.engine.on_input(
            DeviceInput::BehaviorAction(BehaviorActionInput { action_type }),
            self.bpm as f32,
        )?;
        Ok(())
    }

    fn handle_sample_assignment_grid_press(&mut self, x: usize, y: usize) {
        let Some((instrument_slot, sample_slot)) = self.sample_assign else {
            return;
        };
        let mut points = Vec::new();
        if self.ui.fn_held && self.ui.shift_held {
            for row in 0..GRID_HEIGHT {
                points.push((x, row));
            }
        } else if self.ui.shift_held {
            for col in 0..GRID_WIDTH {
                points.push((col, y));
            }
        } else {
            points.push((x, y));
        }
        for (px, py) in points {
            self.assign_sample_cell(instrument_slot, sample_slot, px, py);
        }
        self.config_dirty = true;
        self.menu.rebuild(self.menu_config());
    }

    fn assign_sample_cell(
        &mut self,
        instrument_slot: usize,
        sample_slot: usize,
        x: usize,
        y: usize,
    ) {
        let Some(instrument) = self.instruments.get_mut(instrument_slot) else {
            return;
        };
        if x >= GRID_WIDTH || y >= GRID_HEIGHT {
            return;
        }
        if let Some(index) = instrument
            .sample_assignments
            .iter()
            .position(|assignment| assignment.x == x && assignment.y == y)
        {
            if instrument.sample_assignments[index].sample_slot == sample_slot {
                if instrument.sample_velocity_levels_enabled {
                    instrument.sample_assignments[index].level =
                        match instrument.sample_assignments[index].level.as_deref() {
                            Some("high") => Some("medium".into()),
                            Some("medium") => Some("low".into()),
                            _ => {
                                let _ = instrument.sample_assignments.remove(index);
                                return;
                            }
                        };
                    return;
                }
                let _ = instrument.sample_assignments.remove(index);
                return;
            }
            instrument.sample_assignments[index].sample_slot = sample_slot;
            instrument.sample_assignments[index].level =
                if instrument.sample_velocity_levels_enabled {
                    Some("high".into())
                } else {
                    None
                };
            return;
        }
        instrument.sample_assignments.push(NativeSampleAssignment {
            x,
            y,
            sample_slot,
            level: if instrument.sample_velocity_levels_enabled {
                Some("high".into())
            } else {
                None
            },
        });
    }

    fn handle_trigger_probability_grid_press(&mut self, x: usize, y: usize) {
        let Some(part_index) = self.trigger_probability_assign else {
            return;
        };
        if x >= GRID_WIDTH || y >= GRID_HEIGHT {
            return;
        }
        let next = self.next_probability_state(part_index, x, y);
        if self.ui.shift_held && self.ui.fn_held {
            for row in 0..GRID_HEIGHT {
                self.set_probability_cell(part_index, x, row, &next);
            }
        } else if self.ui.shift_held {
            for column in 0..GRID_WIDTH {
                self.set_probability_cell(part_index, column, y, &next);
            }
        } else {
            self.set_probability_cell(part_index, x, y, &next);
        }
        self.config_dirty = true;
    }

    fn next_probability_state(&self, part_index: usize, x: usize, y: usize) -> String {
        let current = self
            .trigger_probability_maps
            .get(part_index)
            .and_then(|map| map.get(y * GRID_WIDTH + x))
            .map(String::as_str)
            .unwrap_or("zero");
        match current {
            "zero" => "low",
            "low" => "high",
            "high" => "full",
            _ => "zero",
        }
        .into()
    }

    fn set_probability_cell(&mut self, part_index: usize, x: usize, y: usize, value: &str) {
        let Some(map) = self.trigger_probability_maps.get_mut(part_index) else {
            return;
        };
        if let Some(cell) = map.get_mut(y * GRID_WIDTH + x) {
            *cell = value.into();
        }
    }

    fn enter_root_group(&mut self, label: Option<&str>) {
        match label {
            Some("L4: Dance") => {
                self.active_dance_mode = self.dance_mode.clone();
            }
            Some("L1: Life") => {
                self.menu.state.cursor = self.active_part_index.min(GRID_HEIGHT.saturating_sub(1));
                self.active_dance_mode = "none".into();
            }
            Some("L2: Sense") => {
                self.menu.state.cursor = (self.active_part_index + 1).min(GRID_HEIGHT);
                self.active_dance_mode = "none".into();
            }
            Some("L3: Voice") | Some("System") => {
                self.active_dance_mode = "none".into();
            }
            _ => {}
        }
    }

    fn enter_nested_group(
        &mut self,
        stack_depth_before: usize,
        label: Option<&str>,
    ) -> Result<(), String> {
        if stack_depth_before == 1 {
            if let Some(label) = label {
                if let Some(part) = label
                    .strip_prefix('P')
                    .and_then(|rest| rest.split(':').next())
                {
                    if let Ok(index) = part.parse::<usize>() {
                        self.select_active_part(index.saturating_sub(1))?;
                    }
                }
            }
        }
        Ok(())
    }

    fn seed_visible_state(&mut self) -> Result<(), String> {
        match self.behavior.id() {
            "life" => {
                self.engine
                    .on_input(DeviceInput::GridPress { x: 2, y: 3 }, self.bpm as f32)?;
                self.engine
                    .on_input(DeviceInput::GridPress { x: 3, y: 3 }, self.bpm as f32)?;
                self.engine
                    .on_input(DeviceInput::GridPress { x: 4, y: 3 }, self.bpm as f32)?;
            }
            "glider" => {
                self.trigger_behavior_action("spawnGlider".into())?;
            }
            _ => {}
        }
        Ok(())
    }
}

fn velocity_curve_id(curve: VelocityCurve) -> &'static str {
    match curve {
        VelocityCurve::Linear => "linear",
        VelocityCurve::Soft => "soft",
        VelocityCurve::Hard => "hard",
    }
}

fn velocity_curve_from_id(value: &str) -> VelocityCurve {
    match value {
        "soft" => VelocityCurve::Soft,
        "hard" => VelocityCurve::Hard,
        _ => VelocityCurve::Linear,
    }
}

fn scale_steps(scale: &str, root: &str) -> Vec<i32> {
    let intervals = match scale {
        "major" => &[0, 2, 4, 5, 7, 9, 11][..],
        "natural_minor" => &[0, 2, 3, 5, 7, 8, 10][..],
        "dorian" => &[0, 2, 3, 5, 7, 9, 10][..],
        "mixolydian" => &[0, 2, 4, 5, 7, 9, 10][..],
        "major_pentatonic" => &[0, 2, 4, 7, 9][..],
        "minor_pentatonic" => &[0, 3, 5, 7, 10][..],
        "harmonic_minor" => &[0, 2, 3, 5, 7, 8, 11][..],
        _ => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][..],
    };
    let root_offset = match root {
        "C#" => 1,
        "D" => 2,
        "D#" => 3,
        "E" => 4,
        "F" => 5,
        "F#" => 6,
        "G" => 7,
        "G#" => 8,
        "A" => 9,
        "A#" => 10,
        "B" => 11,
        _ => 0,
    };
    intervals
        .iter()
        .map(|step| (step + root_offset) % 12)
        .collect()
}

fn display_index(x: usize, y: usize) -> usize {
    (GRID_HEIGHT - 1 - y) * GRID_WIDTH + x
}

fn display_part_index_from_y(y: usize) -> usize {
    y.min(GRID_HEIGHT - 1)
}

fn dance_fx_cell_id(x: usize, y: usize) -> String {
    format!("momentary-fx:{x}:{y}")
}

fn momentary_fx_target(value: &str) -> RuntimeMomentaryFxTarget {
    if let Some(index) = value
        .strip_prefix("fx_bus_")
        .and_then(|value| value.parse::<usize>().ok())
        .and_then(|value| value.checked_sub(1))
    {
        return RuntimeMomentaryFxTarget::FxBus { index };
    }
    if let Some(index) = value
        .strip_prefix("instrument_")
        .and_then(|value| value.parse::<usize>().ok())
        .and_then(|value| value.checked_sub(1))
    {
        return RuntimeMomentaryFxTarget::Instrument { index };
    }
    RuntimeMomentaryFxTarget::Global
}

fn sanitize_pan_position_payload(raw: u64, incoming_pan_positions: Option<u64>) -> u8 {
    if incoming_pan_positions == Some(u64::from(PAN_POSITION_COUNT)) {
        return (raw as u8).min(PAN_POSITION_COUNT - 1);
    }
    if incoming_pan_positions == Some(GRID_WIDTH as u64)
        || (incoming_pan_positions.is_none() && raw < GRID_WIDTH as u64)
    {
        let old_center_left = (GRID_WIDTH - 1) / 2;
        let old_center_right = GRID_WIDTH / 2;
        if raw as usize == old_center_left || raw as usize == old_center_right {
            return PAN_POSITION_COUNT / 2;
        }
        return (((raw.min((GRID_WIDTH - 1) as u64) as f32 / (GRID_WIDTH - 1) as f32)
            * f32::from(PAN_POSITION_COUNT - 1))
        .round()) as u8;
    }
    (raw as u8).min(PAN_POSITION_COUNT - 1)
}

fn pan_marker_left_cell(pan_pos: u8) -> usize {
    (((pan_pos.min(PAN_POSITION_COUNT - 1)) as f32 / f32::from(PAN_POSITION_COUNT - 1))
        * (GRID_WIDTH - 2) as f32)
        .round()
        .clamp(0.0, (GRID_WIDTH - 2) as f32) as usize
}

fn fx_bus_configs(buses: &[NativeFxBus]) -> Vec<NativeFxBusConfig> {
    buses
        .iter()
        .map(|bus| NativeFxBusConfig {
            name: bus.name.clone(),
            slot1_type: bus.slot1_type.clone(),
            slot1_params: bus.slot1_params.clone(),
            slot2_type: bus.slot2_type.clone(),
            slot2_params: bus.slot2_params.clone(),
            pan_pos: bus.pan_pos,
            auto_name: bus.auto_name,
        })
        .collect()
}

fn sense_part_configs(parts: &[NativeSensePart]) -> Vec<NativeSensePartConfig> {
    parts
        .iter()
        .map(|part| NativeSensePartConfig {
            scan_mode: part.scan_mode.clone(),
            scan_axis: part.scan_axis.clone(),
            scan_unit: part.scan_unit.clone(),
            scan_direction: part.scan_direction.clone(),
            scan_sections: part.scan_sections,
            scanned_slot: part.scanned_slot,
            scanned_action: part.scanned_action.clone(),
            scanned_empty_slot: part.scanned_empty_slot,
            scanned_empty_action: part.scanned_empty_action.clone(),
            event_enabled: part.event_enabled,
            activate_slot: part.activate_slot,
            activate_action: part.activate_action.clone(),
            stable_slot: part.stable_slot,
            stable_action: part.stable_action.clone(),
            deactivate_slot: part.deactivate_slot,
            deactivate_action: part.deactivate_action.clone(),
            trigger_probability_mode: part.trigger_probability_mode.clone(),
            trigger_probability_low_pct: part.trigger_probability_low_pct,
            trigger_probability_high_pct: part.trigger_probability_high_pct,
            state_notes_enabled: part.state_notes_enabled,
            lowest_note: part.lowest_note,
            highest_note: part.highest_note,
            starting_note: part.starting_note,
            scale: part.scale.clone(),
            root: part.root.clone(),
            out_of_range: part.out_of_range.clone(),
            x_pitch_enabled: part.x_pitch_enabled,
            x_pitch_steps: part.x_pitch_steps,
            x_pitch_restart_each_section: part.x_pitch_restart_each_section,
            y_pitch_enabled: part.y_pitch_enabled,
            y_pitch_steps: part.y_pitch_steps,
            y_pitch_restart_each_section: part.y_pitch_restart_each_section,
            x_from: part.x_from,
            x_to: part.x_to,
            x_velocity: value_lane_config(&part.x_velocity),
            x_filter_cutoff: value_lane_config(&part.x_filter_cutoff),
            x_filter_resonance: value_lane_config(&part.x_filter_resonance),
            y_from: part.y_from,
            y_to: part.y_to,
            y_velocity: value_lane_config(&part.y_velocity),
            y_filter_cutoff: value_lane_config(&part.y_filter_cutoff),
            y_filter_resonance: value_lane_config(&part.y_filter_resonance),
        })
        .collect()
}

fn value_lane_config(lane: &NativeValueLane) -> NativeValueLaneConfig {
    NativeValueLaneConfig {
        enabled: lane.enabled,
        from: lane.from,
        to: lane.to,
        grid_offset: lane.grid_offset,
        curve: lane.curve.clone(),
    }
}

fn sense_part_payload(part: &NativeSensePart, probability_map: &[String]) -> Value {
    json!({
        "scanMode": part.scan_mode.clone(),
        "scanAxis": part.scan_axis.clone(),
        "scanUnit": part.scan_unit.clone(),
        "scanDirection": part.scan_direction.clone(),
        "scanSections": part.scan_sections,
        "eventEnabled": part.event_enabled,
        "triggerProbabilityMode": part.trigger_probability_mode.clone(),
        "triggerProbabilityLowPct": part.trigger_probability_low_pct,
        "triggerProbabilityHighPct": part.trigger_probability_high_pct,
        "stateNotesEnabled": part.state_notes_enabled,
        "triggerProbabilityMap": probability_map,
        "mapping": {
            "scanned": { "slot": slot_payload(part.scanned_slot), "action": part.scanned_action.clone() },
            "scanned_empty": { "slot": slot_payload(part.scanned_empty_slot), "action": part.scanned_empty_action.clone() },
            "activate": { "slot": slot_payload(part.activate_slot), "action": part.activate_action.clone() },
            "stable": { "slot": slot_payload(part.stable_slot), "action": part.stable_action.clone() },
            "deactivate": { "slot": slot_payload(part.deactivate_slot), "action": part.deactivate_action.clone() }
        },
        "pitch": {
            "lowestNote": part.lowest_note,
            "highestNote": part.highest_note,
            "startingNote": part.starting_note,
            "scale": part.scale.clone(),
            "root": part.root.clone(),
            "outOfRange": part.out_of_range.clone()
        },
        "x": {
            "from": part.x_from,
            "to": part.x_to,
            "pitch": {
                "enabled": part.x_pitch_enabled,
                "steps": part.x_pitch_steps,
                "restartEachSection": part.x_pitch_restart_each_section
            },
            "velocity": value_lane_payload(&part.x_velocity),
            "filterCutoff": value_lane_payload(&part.x_filter_cutoff),
            "filterResonance": value_lane_payload(&part.x_filter_resonance)
        },
        "y": {
            "from": part.y_from,
            "to": part.y_to,
            "pitch": {
                "enabled": part.y_pitch_enabled,
                "steps": part.y_pitch_steps,
                "restartEachSection": part.y_pitch_restart_each_section
            },
            "velocity": value_lane_payload(&part.y_velocity),
            "filterCutoff": value_lane_payload(&part.y_filter_cutoff),
            "filterResonance": value_lane_payload(&part.y_filter_resonance)
        }
    })
}

fn value_lane_payload(lane: &NativeValueLane) -> Value {
    json!({
        "enabled": lane.enabled,
        "from": lane.from,
        "to": lane.to,
        "gridOffset": lane.grid_offset,
        "curve": lane.curve
    })
}

fn param_mods_payload(param_mods: Option<&NativeParamMods>) -> Value {
    let empty = NativeParamMods::default();
    let param_mods = param_mods.unwrap_or(&empty);
    json!({
        "x": [param_binding_payload(param_mods.x.first().and_then(Option::as_ref)), param_binding_payload(param_mods.x.get(1).and_then(Option::as_ref))],
        "y": [param_binding_payload(param_mods.y.first().and_then(Option::as_ref)), param_binding_payload(param_mods.y.get(1).and_then(Option::as_ref))]
    })
}

fn param_mod_configs(param_mods: &[NativeParamMods]) -> Vec<NativeParamModsConfig> {
    param_mods
        .iter()
        .map(|mods| NativeParamModsConfig {
            x: [
                mods.x
                    .first()
                    .and_then(Option::as_ref)
                    .map(param_binding_spec_from_native),
                mods.x
                    .get(1)
                    .and_then(Option::as_ref)
                    .map(param_binding_spec_from_native),
            ],
            y: [
                mods.y
                    .first()
                    .and_then(Option::as_ref)
                    .map(param_binding_spec_from_native),
                mods.y
                    .get(1)
                    .and_then(Option::as_ref)
                    .map(param_binding_spec_from_native),
            ],
        })
        .collect()
}

fn aux_binding_configs(bindings: &[Option<NativeAuxBinding>]) -> Vec<NativeAuxBindingConfig> {
    bindings
        .iter()
        .map(|binding| NativeAuxBindingConfig {
            turn: binding
                .as_ref()
                .and_then(|binding| binding.turn_key.as_ref())
                .map(|key| NativeParamBindingSpec {
                    key: key.clone(),
                    label: None,
                    kind: "number".into(),
                    min: None,
                    max: None,
                    step: None,
                    options: vec![],
                    invert: false,
                }),
            click: binding
                .as_ref()
                .and_then(|binding| binding.press_action.clone()),
        })
        .collect()
}

fn param_binding_spec_from_native(binding: &NativeParamBinding) -> NativeParamBindingSpec {
    NativeParamBindingSpec {
        key: binding.key.clone(),
        label: binding.label.clone(),
        kind: binding.kind.clone(),
        min: binding.min.map(|value| value as i32),
        max: binding.max.map(|value| value as i32),
        step: binding.step.map(|value| value as i32),
        options: binding.options.clone(),
        invert: binding.invert,
    }
}

fn native_binding_from_spec(binding: NativeParamBindingSpec) -> NativeParamBinding {
    NativeParamBinding {
        key: binding.key,
        label: binding.label,
        kind: binding.kind,
        min: binding.min.map(f64::from),
        max: binding.max.map(f64::from),
        step: binding.step.map(f64::from),
        options: binding.options,
        invert: binding.invert,
    }
}

fn remap_behavior_param_binding(
    binding: NativeParamBinding,
    to_behavior: NativeBehavior,
    part_index: usize,
) -> Option<NativeParamBinding> {
    let remapped = remap_behavior_binding_key(&binding.key, to_behavior, Some(part_index))?;
    Some(NativeParamBinding {
        invert: binding.invert,
        ..remapped
    })
}

fn remap_behavior_binding_key(
    key: &str,
    to_behavior: NativeBehavior,
    part_index: Option<usize>,
) -> Option<NativeParamBinding> {
    if let Some((index, param_key)) = parse_part_behavior_config_binding_key(key) {
        let analogue = behavior_param_analogue(param_key, to_behavior)?;
        return Some(NativeParamBinding {
            key: format!(
                "parts.{}.l1.behaviorConfig.{}",
                part_index.unwrap_or(index),
                analogue.key
            ),
            ..analogue
        });
    }
    let rest = key.strip_prefix("behaviorConfig.")?;
    let (_, param_key) = rest.split_once('.')?;
    let analogue = behavior_param_analogue(param_key, to_behavior)?;
    Some(NativeParamBinding {
        key: format!("behaviorConfig.{}.{}", to_behavior.id(), analogue.key),
        ..analogue
    })
}

fn behavior_param_analogue(
    param_key: &str,
    behavior: NativeBehavior,
) -> Option<NativeParamBinding> {
    let state = behavior.init(Value::Null).ok()?;
    let items = behavior.config_menu(&state).ok()??;
    let keys = behavior_param_analogue_keys(param_key);
    for item in items {
        if !keys.iter().any(|key| *key == item.key) {
            continue;
        }
        return match item.item_type {
            BehaviorConfigItemType::Number => Some(NativeParamBinding {
                key: item.key,
                label: Some(item.label),
                kind: "number".into(),
                min: Some(f64::from(item.min.unwrap_or(0))),
                max: Some(f64::from(item.max.unwrap_or(127))),
                step: Some(f64::from(item.step.unwrap_or(1))),
                options: vec![],
                invert: false,
            }),
            BehaviorConfigItemType::Enum => Some(NativeParamBinding {
                key: item.key,
                label: Some(item.label),
                kind: "enum".into(),
                min: None,
                max: None,
                step: None,
                options: item.options.unwrap_or_default(),
                invert: false,
            }),
            BehaviorConfigItemType::Bool => Some(NativeParamBinding {
                key: item.key,
                label: Some(item.label),
                kind: "bool".into(),
                min: None,
                max: None,
                step: None,
                options: vec![],
                invert: false,
            }),
            BehaviorConfigItemType::Action => None,
        };
    }
    None
}

fn behavior_param_analogue_keys(param_key: &str) -> Vec<&str> {
    const GROUPS: &[&[&str]] = &[
        &[
            "randomTickInterval",
            "seedInterval",
            "autoSpawnInterval",
            "spawnInterval",
            "autoPulseInterval",
            "autoDropInterval",
        ],
        &[
            "randomCellsPerTick",
            "randomSeedCells",
            "maxAnts",
            "maxBalls",
        ],
    ];
    GROUPS
        .iter()
        .find(|group| group.contains(&param_key))
        .map(|group| group.to_vec())
        .unwrap_or_else(|| vec![param_key])
}

fn primary_behavior_action(behavior: NativeBehavior) -> Option<(String, String)> {
    let state = behavior.init(Value::Null).ok()?;
    let items = behavior.config_menu(&state).ok()??;
    items.into_iter().find_map(|item| {
        if item.item_type == BehaviorConfigItemType::Action {
            Some((item.key, item.label))
        } else {
            None
        }
    })
}

fn param_binding_payload(binding: Option<&NativeParamBinding>) -> Value {
    let Some(binding) = binding else {
        return Value::Null;
    };
    let mut value = serde_json::Map::new();
    value.insert("key".into(), json!(binding.key));
    if let Some(label) = &binding.label {
        value.insert("label".into(), json!(label));
    }
    value.insert("kind".into(), json!(binding.kind));
    if let Some(min) = binding.min {
        value.insert("min".into(), json!(min));
    }
    if let Some(max) = binding.max {
        value.insert("max".into(), json!(max));
    }
    if let Some(step) = binding.step {
        value.insert("step".into(), json!(step));
    }
    if !binding.options.is_empty() {
        value.insert("options".into(), json!(binding.options));
    }
    value.insert("invert".into(), json!(binding.invert));
    Value::Object(value)
}

fn param_mods_from_payload(payload: &Value) -> NativeParamMods {
    NativeParamMods {
        x: param_axis_bindings_from_payload(payload.get("x")),
        y: param_axis_bindings_from_payload(payload.get("y")),
    }
}

fn param_axis_bindings_from_payload(payload: Option<&Value>) -> Vec<Option<NativeParamBinding>> {
    let mut out = vec![None, None];
    if let Some(values) = payload.and_then(Value::as_array) {
        for (index, value) in values.iter().take(2).enumerate() {
            out[index] = param_binding_from_payload(value);
        }
    }
    out
}

fn param_binding_from_payload(payload: &Value) -> Option<NativeParamBinding> {
    let key = payload.get("key")?.as_str()?.to_string();
    if !supported_param_binding_key(&key) {
        return None;
    }
    let kind = match payload.get("kind").and_then(Value::as_str) {
        Some("enum") => "enum",
        Some("bool") => "bool",
        _ => "number",
    }
    .to_string();
    Some(NativeParamBinding {
        key,
        label: payload
            .get("label")
            .and_then(Value::as_str)
            .map(str::to_string),
        kind,
        min: payload.get("min").and_then(Value::as_f64),
        max: payload.get("max").and_then(Value::as_f64),
        step: payload.get("step").and_then(Value::as_f64),
        options: payload
            .get("options")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        invert: payload
            .get("invert")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn supported_param_binding_key(key: &str) -> bool {
    if matches!(
        key,
        "sound.noteLengthMs" | "sound.velocityScalePct" | "sound.voiceStealingMode"
    ) || key.starts_with("parts.") && key.contains(".l1.behaviorConfig.")
    {
        return true;
    }
    let Some((_, field)) = parse_instrument_binding_key(key) else {
        return false;
    };
    matches!(
        field,
        "mixer.volume"
            | "mixer.panPos"
            | "synth.amp.gainPct"
            | "sample.tuneSemis"
            | "sample.amp.gainPct"
            | "sample.amp.velocitySensitivityPct"
            | "sample.baseVelocity"
            | "midi.enabled"
            | "midi.channel"
            | "midi.velocity"
            | "midi.durationMs"
    )
}

fn supported_aux_turn_key(key: &str) -> bool {
    !key.is_empty()
        && !key.contains("..")
        && (supported_param_binding_key(key)
            || key.starts_with("parts.")
            || key.starts_with("mixer.")
            || key.starts_with("transport.")
            || key.starts_with("dance.")
            || key.starts_with("midi")
            || key.starts_with("screen")
            || key.ends_with("Brightness")
            || matches!(
                key,
                "autoSaveDefault" | "ghostCells" | "inputEventsWhilePaused" | "numericDisplayMode"
            ))
}

fn native_factory_payload() -> Value {
    let mut parts = Vec::new();
    for index in 0..GRID_HEIGHT {
        let behavior_id = match index {
            0 => "life",
            1 => "sequencer",
            _ => "none",
        };
        let mut sense = NativeSensePart::default();
        if index == 0 {
            sense.scan_axis = "columns".into();
            sense.event_enabled = true;
            sense.activate_action = "note_on".into();
            sense.stable_action = "none".into();
            sense.deactivate_action = "note_off".into();
        } else if index == 1 {
            sense.scan_axis = "rows".into();
            sense.event_enabled = true;
            sense.activate_action = "none".into();
            sense.stable_action = "none".into();
            sense.deactivate_action = "none".into();
            sense.scanned_slot = 1;
            sense.scanned_action = "note_on".into();
            sense.scanned_empty_slot = 1;
            sense.scanned_empty_action = "note_off".into();
        } else {
            sense.event_enabled = false;
            sense.activate_action = "none".into();
            sense.stable_action = "none".into();
            sense.deactivate_action = "none".into();
            sense.scanned_action = "none".into();
            sense.scanned_empty_action = "none".into();
        }
        parts.push(json!({
            "l1": {
                "behaviorId": behavior_id,
                "stepRate": if index == 1 { "1/4" } else { "1/8" },
                "behaviorConfig": if index == 0 { json!({ "randomCellsPerTick": 12, "randomTickInterval": 1 }) } else { json!({}) },
                "saveGridState": true
            },
            "l2": sense_part_payload(&sense, &vec!["full".into(); GRID_WIDTH * GRID_HEIGHT]),
            "autoName": true,
            "name": behavior_id
        }));
    }
    json!({
        "activeBehavior": "life",
        "runtimeConfig": {
            "activeBehavior": "life",
            "activePartIndex": 0,
            "parts": parts,
            "instruments": [
                { "type": "synth", "noteBehavior": "oneshot", "autoName": true, "name": "synth", "synth": synth_preset_config("init"), "sample": { "selectedSlot": 0, "slots": [], "assignments": [], "tuneSemis": 0, "amp": { "gainPct": 100 } }, "midi": { "enabled": false, "velocity": 100, "durationMs": 120 }, "mixer": { "route": "fx_bus_1", "panPos": 16, "volume": 100 } },
                { "type": "synth", "noteBehavior": "oneshot", "autoName": true, "name": "drums", "synth": synth_preset_config("init"), "sample": { "selectedSlot": 0, "slots": [], "assignments": [], "tuneSemis": 0, "amp": { "gainPct": 100 } }, "midi": { "enabled": false, "velocity": 100, "durationMs": 120 }, "mixer": { "route": "direct", "panPos": 16, "volume": 100 } }
            ],
            "mixer": {
                "buses": [{ "slot1": { "type": "delay" }, "slot2": { "type": "duck" }, "panPos": 16, "autoName": true }],
                "master": { "slots": [{ "type": "none" }, { "type": "none" }] }
            },
            "danceMode": "none",
            "autoSaveDefault": false
        },
        "mappingConfig": default_mapping_config()
    })
}

fn sample_assignments_payload(assignments: &[NativeSampleAssignment]) -> Value {
    Value::Array(
        assignments
            .iter()
            .map(|assignment| {
                json!({
                    "x": assignment.x,
                    "y": assignment.y,
                    "sampleSlot": assignment.sample_slot,
                    "level": assignment.level,
                })
            })
            .collect(),
    )
}

fn sample_assignment_from_payload(value: &Value) -> Option<NativeSampleAssignment> {
    let level = value
        .get("level")
        .and_then(Value::as_str)
        .and_then(|level| {
            if matches!(level, "high" | "medium" | "low") {
                Some(level.to_string())
            } else {
                None
            }
        });
    Some(NativeSampleAssignment {
        x: (value.get("x")?.as_u64()? as usize).min(GRID_WIDTH - 1),
        y: (value.get("y")?.as_u64()? as usize).min(GRID_HEIGHT - 1),
        sample_slot: (value.get("sampleSlot")?.as_u64()? as usize).min(7),
        level,
    })
}

fn apply_trigger_probability_map_payload(target: &mut [String], map: &[Value]) {
    for (cell_index, value) in map.iter().take(GRID_WIDTH * GRID_HEIGHT).enumerate() {
        if let Some(value) = value.as_str() {
            if matches!(value, "zero" | "low" | "high" | "full") {
                if let Some(cell) = target.get_mut(cell_index) {
                    *cell = value.into();
                }
            }
        }
    }
}

fn apply_legacy_trigger_gates_payload(target: &mut [String], gates: &[Value]) {
    for (cell_index, value) in gates.iter().take(GRID_WIDTH * GRID_HEIGHT).enumerate() {
        if let Some(cell) = target.get_mut(cell_index) {
            *cell = if value.as_bool() == Some(false) {
                "zero".into()
            } else {
                "full".into()
            };
        }
    }
}

fn aux_bindings_payload(bindings: &[Option<NativeAuxBinding>]) -> Value {
    let mut object = serde_json::Map::new();
    for (index, binding) in bindings.iter().enumerate() {
        let key = format!("aux{}", index + 1);
        let value = if let Some(binding) = binding {
            json!({
                "turnKey": binding.turn_key.clone(),
                "pressAction": match &binding.press_action {
                    Some(NativeMenuAction::BehaviorAction(action)) => json!({ "kind": "behavior_action", "actionType": action.clone() }),
                    Some(NativeMenuAction::PlatformEffect(action)) => json!({ "kind": "platform_effect", "action": action.clone() }),
                    Some(NativeMenuAction::CloneInstrument { index }) => json!({ "kind": "instrument_clone", "slot": index }),
                    Some(NativeMenuAction::ResetInstrument { index }) => json!({ "kind": "instrument_reset", "slot": index }),
                    Some(NativeMenuAction::ResetBehavior) => json!({ "kind": "reset_behavior" }),
                    _ => Value::Null,
                }
            })
        } else {
            Value::Null
        };
        object.insert(key, value);
    }
    Value::Object(object)
}

fn instrument_labels(instruments: &[NativeInstrumentSlot]) -> Vec<String> {
    instruments
        .iter()
        .enumerate()
        .map(|(index, instrument)| format!("I{}: {}", index + 1, instrument.name))
        .collect()
}

fn instrument_names(instruments: &[NativeInstrumentSlot]) -> Vec<String> {
    instruments
        .iter()
        .map(|instrument| instrument.name.clone())
        .collect()
}

fn instrument_auto_names(instruments: &[NativeInstrumentSlot]) -> Vec<bool> {
    instruments
        .iter()
        .map(|instrument| instrument.auto_name)
        .collect()
}

fn instrument_note_behaviors(instruments: &[NativeInstrumentSlot]) -> Vec<String> {
    instruments
        .iter()
        .map(|instrument| instrument.note_behavior.clone())
        .collect()
}

fn note_behaviors_from_instruments(instruments: &[NativeInstrumentSlot]) -> Vec<NoteBehavior> {
    let mut note_behaviors = vec![NoteBehavior::Oneshot; 16];
    for (index, instrument) in instruments.iter().enumerate().take(note_behaviors.len()) {
        note_behaviors[index] = if instrument.note_behavior == "hold" {
            NoteBehavior::Hold
        } else {
            NoteBehavior::Oneshot
        };
    }
    note_behaviors
}

fn instrument_types(instruments: &[NativeInstrumentSlot]) -> Vec<String> {
    instruments
        .iter()
        .map(|instrument| instrument.kind.clone())
        .collect()
}

fn instrument_routes(instruments: &[NativeInstrumentSlot]) -> Vec<String> {
    instruments
        .iter()
        .map(|instrument| instrument.route.clone())
        .collect()
}

fn instrument_volumes(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.volume)
        .collect()
}

fn instrument_pan_positions(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.pan_pos)
        .collect()
}

fn normalize_route(route: &str) -> String {
    route
        .strip_prefix("bus_")
        .map(|suffix| format!("fx_bus_{suffix}"))
        .unwrap_or_else(|| route.to_string())
}

fn trigger_target(slot: usize, action: &str, velocity: u8, duration_ms: u32) -> TriggerTarget {
    let action = if slot >= INSTRUMENT_COUNT {
        TriggerAction::None
    } else {
        match action {
            "note_off" => TriggerAction::NoteOff,
            "none" => TriggerAction::None,
            _ => TriggerAction::NoteOn,
        }
    };
    TriggerTarget {
        action,
        channel: slot.min(15) as u8,
        velocity,
        duration_ms,
    }
}

fn slot_payload(slot: usize) -> Value {
    if slot >= INSTRUMENT_COUNT {
        Value::String("none".into())
    } else {
        Value::String(slot.to_string())
    }
}

fn instrument_sample_slots(instruments: &[NativeInstrumentSlot]) -> Vec<usize> {
    instruments
        .iter()
        .map(|instrument| instrument.selected_sample_slot)
        .collect()
}

fn instrument_synth_configs(instruments: &[NativeInstrumentSlot]) -> Vec<Value> {
    instruments
        .iter()
        .map(|instrument| instrument.synth_config.clone())
        .collect()
}

fn instrument_synth_osc1_waveforms(instruments: &[NativeInstrumentSlot]) -> Vec<String> {
    instruments
        .iter()
        .map(|instrument| synth_string_at(instrument, &["osc1", "waveform"], "saw"))
        .collect()
}

fn instrument_synth_osc2_waveforms(instruments: &[NativeInstrumentSlot]) -> Vec<String> {
    instruments
        .iter()
        .map(|instrument| synth_string_at(instrument, &["osc2", "waveform"], "square"))
        .collect()
}

fn instrument_synth_filter_types(instruments: &[NativeInstrumentSlot]) -> Vec<String> {
    instruments
        .iter()
        .map(|instrument| synth_string_at(instrument, &["filter", "type"], "lowpass"))
        .collect()
}

fn instrument_synth_filter_cutoffs(instruments: &[NativeInstrumentSlot]) -> Vec<u16> {
    instruments.iter().map(synth_filter_cutoff).collect()
}

fn instrument_synth_gain_pct(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.synth_gain_pct)
        .collect()
}

fn instrument_synth_filter_resonance(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments.iter().map(synth_filter_resonance).collect()
}

fn synth_filter_resonance(instrument: &NativeInstrumentSlot) -> u8 {
    instrument
        .synth_config
        .get("filter")
        .and_then(|filter| filter.get("resonance"))
        .and_then(Value::as_u64)
        .unwrap_or(20)
        .min(255) as u8
}

fn synth_filter_cutoff(instrument: &NativeInstrumentSlot) -> u16 {
    instrument
        .synth_config
        .get("filter")
        .and_then(|filter| filter.get("cutoffHz"))
        .and_then(Value::as_u64)
        .unwrap_or(8000)
        .clamp(20, 20000) as u16
}

fn synth_i32_at(instrument: &NativeInstrumentSlot, path: &[&str], fallback: i32) -> i32 {
    value_i32_at(&instrument.synth_config, path, fallback)
}

fn synth_string_at(instrument: &NativeInstrumentSlot, path: &[&str], fallback: &str) -> String {
    value_string_at(&instrument.synth_config, path, fallback)
}

fn cutoff_display_to_hz(display: i32) -> i32 {
    let t = f64::from(display.clamp(0, 255)) / 255.0;
    (80.0 * (16_000.0_f64 / 80.0).ln().mul_add(t, 0.0).exp()).round() as i32
}

fn cutoff_hz_to_display(hz: i32) -> i32 {
    let h = hz.clamp(80, 16_000) as f64;
    ((h / 80.0).ln() / (16_000.0_f64 / 80.0).ln() * 255.0).round() as i32
}

fn value_i32_at(value: &Value, path: &[&str], fallback: i32) -> i32 {
    let mut current = value;
    for key in path {
        let Some(next) = current.get(*key) else {
            return fallback;
        };
        current = next;
    }
    current.as_i64().unwrap_or(i64::from(fallback)) as i32
}

fn value_string_at(value: &Value, path: &[&str], fallback: &str) -> String {
    let mut current = value;
    for key in path {
        let Some(next) = current.get(*key) else {
            return fallback.into();
        };
        current = next;
    }
    current.as_str().unwrap_or(fallback).into()
}

fn set_json_path_string(value: &mut Value, path: &[&str], text: &str) {
    let Some((last, parents)) = path.split_last() else {
        return;
    };
    let mut current = value;
    for key in parents {
        let Some(object) = current.as_object_mut() else {
            return;
        };
        let Some(next) = object.get_mut(*key) else {
            return;
        };
        current = next;
    }
    if let Some(object) = current.as_object_mut() {
        object.insert((*last).to_string(), json!(text));
    }
}

fn set_json_path_number(value: &mut Value, path: &[&str], number: f64) {
    let Some((last, parents)) = path.split_last() else {
        return;
    };
    let mut current = value;
    for key in parents {
        let Some(object) = current.as_object_mut() else {
            return;
        };
        let Some(next) = object.get_mut(*key) else {
            return;
        };
        current = next;
    }
    if let Some(object) = current.as_object_mut() {
        object.insert((*last).to_string(), json!(number.round() as i64));
    }
}

fn synth_preset_config(id: &str) -> Value {
    match id {
        "soft_pad" => synth_config(
            "triangle", 78, 0, -3, 50, "pulse", 64, 0, 3, 42, 72, 85, 240, 360, 78, 460, "lowpass",
            3800, 18, 28, 20, 190, 420, 72, 500,
        ),
        "bright_pluck" => synth_config(
            "saw", 86, 0, 0, 50, "pulse", 52, 1, 6, 30, 84, 100, 3, 120, 18, 70, "lowpass", 7200,
            34, 54, 34, 2, 180, 16, 120,
        ),
        "bass_mono" => synth_config(
            "saw", 84, -1, 0, 50, "square", 68, -1, -4, 50, 88, 72, 5, 160, 56, 120, "lowpass",
            2100, 30, 22, 24, 7, 170, 44, 150,
        ),
        "hollow_pwm" => synth_config(
            "pulse", 74, 0, -6, 34, "pulse", 74, 0, 6, 66, 82, 96, 9, 260, 60, 180, "bandpass",
            2500, 48, 30, 28, 5, 220, 40, 180,
        ),
        "lead" => synth_config(
            "saw", 88, 0, 5, 50, "triangle", 64, 1, -2, 50, 85, 100, 2, 130, 26, 110, "highpass",
            650, 24, 46, 30, 3, 140, 24, 130,
        ),
        "bell" => synth_config(
            "sine", 76, 0, 0, 50, "triangle", 60, 1, 12, 50, 76, 100, 1, 540, 0, 360, "notch",
            3000, 52, 34, 12, 1, 380, 0, 280,
        ),
        "perc_hit" => synth_config(
            "square", 84, 0, 0, 50, "pulse", 48, 1, 0, 20, 88, 100, 0, 90, 0, 120, "lowpass", 4200,
            26, 72, 8, 0, 120, 0, 140,
        ),
        _ => synth_config(
            "saw", 80, 0, 0, 50, "square", 72, 0, 0, 50, 80, 100, 5, 120, 70, 180, "lowpass", 8000,
            20, 0, 0, 5, 120, 70, 180,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn synth_config(
    osc1_wave: &str,
    osc1_level: i32,
    osc1_octave: i32,
    osc1_detune: i32,
    osc1_pulse_width: i32,
    osc2_wave: &str,
    osc2_level: i32,
    osc2_octave: i32,
    osc2_detune: i32,
    osc2_pulse_width: i32,
    gain: i32,
    velocity_sensitivity: i32,
    amp_attack: i32,
    amp_decay: i32,
    amp_sustain: i32,
    amp_release: i32,
    filter_type: &str,
    cutoff: i32,
    resonance: i32,
    env_amount: i32,
    key_tracking: i32,
    filter_attack: i32,
    filter_decay: i32,
    filter_sustain: i32,
    filter_release: i32,
) -> Value {
    json!({
        "osc1": { "waveform": osc1_wave, "levelPct": osc1_level, "octave": osc1_octave, "detuneCents": osc1_detune, "pulseWidthPct": osc1_pulse_width },
        "osc2": { "waveform": osc2_wave, "levelPct": osc2_level, "octave": osc2_octave, "detuneCents": osc2_detune, "pulseWidthPct": osc2_pulse_width },
        "amp": { "gainPct": gain, "velocitySensitivityPct": velocity_sensitivity },
        "ampEnv": { "attackMs": amp_attack, "decayMs": amp_decay, "sustainPct": amp_sustain, "releaseMs": amp_release },
        "filter": { "type": filter_type, "cutoffHz": cutoff, "resonance": resonance, "envAmountPct": env_amount, "keyTrackingPct": key_tracking },
        "filterEnv": { "attackMs": filter_attack, "decayMs": filter_decay, "sustainPct": filter_sustain, "releaseMs": filter_release }
    })
}

fn instrument_sample_tune_semis(instruments: &[NativeInstrumentSlot]) -> Vec<i8> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_tune_semis)
        .collect()
}

fn instrument_sample_gain_pct(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_gain_pct)
        .collect()
}

fn instrument_sample_base_velocity(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_base_velocity)
        .collect()
}

fn instrument_sample_amp_velocity_sensitivity_pct(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_amp_velocity_sensitivity_pct)
        .collect()
}

fn instrument_sample_velocity_levels_enabled(instruments: &[NativeInstrumentSlot]) -> Vec<bool> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_velocity_levels_enabled)
        .collect()
}

fn instrument_sample_velocity_high(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_velocity_high)
        .collect()
}

fn instrument_sample_velocity_medium(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_velocity_medium)
        .collect()
}

fn instrument_sample_velocity_low(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_velocity_low)
        .collect()
}

fn instrument_sample_amp_envs(instruments: &[NativeInstrumentSlot]) -> Vec<Value> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_amp_env.clone())
        .collect()
}

fn instrument_sample_filters(instruments: &[NativeInstrumentSlot]) -> Vec<Value> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_filter.clone())
        .collect()
}

fn instrument_sample_filter_envs(instruments: &[NativeInstrumentSlot]) -> Vec<Value> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_filter_env.clone())
        .collect()
}

fn instrument_midi_enabled(instruments: &[NativeInstrumentSlot]) -> Vec<bool> {
    instruments
        .iter()
        .map(|instrument| instrument.midi_enabled)
        .collect()
}

fn instrument_midi_channels(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.midi_channel)
        .collect()
}

fn instrument_midi_velocity(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.midi_velocity)
        .collect()
}

fn instrument_midi_duration_ms(instruments: &[NativeInstrumentSlot]) -> Vec<u16> {
    instruments
        .iter()
        .map(|instrument| instrument.midi_duration_ms)
        .collect()
}

fn parse_sample_action(rest: &str) -> Result<(usize, usize, Option<String>), String> {
    let mut parts = rest.splitn(3, ':');
    let instrument_slot = parts
        .next()
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| format!("invalid sample action `{rest}`"))?;
    let sample_slot = parts
        .next()
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| format!("invalid sample action `{rest}`"))?;
    Ok((
        instrument_slot,
        sample_slot,
        parts.next().map(str::to_string),
    ))
}

fn parent_dir(dir: &str) -> String {
    let mut parts = dir
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let _ = parts.pop();
    parts.join("/")
}

fn set_string_from_menu(menu: &NativeMenuModel, target: &mut String, key: &str) -> bool {
    if let Some(value) = menu.value_for_key(key) {
        if target != &value {
            *target = value;
            return true;
        }
    }
    false
}

fn set_bool_from_menu(menu: &NativeMenuModel, target: &mut bool, key: &str) -> bool {
    if let Some(value) = menu.value_for_key(key).map(|value| value == "true") {
        if *target != value {
            *target = value;
            return true;
        }
    }
    false
}

fn set_target_slot_from_menu(menu: &NativeMenuModel, target: &mut usize, key: &str) -> bool {
    if let Some(value) = menu.value_for_key(key) {
        let parsed = if value == "none" {
            Some(usize::MAX)
        } else {
            parse_slot_index(&value).map(|value| value.min(INSTRUMENT_COUNT - 1))
        };
        if let Some(value) = parsed {
            if *target != value {
                *target = value;
                return true;
            }
        }
    }
    false
}

fn parse_slot_index(value: &str) -> Option<usize> {
    if let Ok(index) = value.parse::<usize>() {
        return Some(index);
    }
    value
        .strip_prefix('I')
        .and_then(|rest| rest.split(':').next())
        .and_then(|number| number.parse::<usize>().ok())
        .and_then(|number| number.checked_sub(1))
}

fn set_u8_from_menu(menu: &NativeMenuModel, target: &mut u8, key: &str, max: u8) -> bool {
    if let Some(value) = menu.number_for_key(key) {
        let value = value.clamp(0, i32::from(max)) as u8;
        if *target != value {
            *target = value;
            return true;
        }
    }
    false
}

fn set_i32_from_menu(
    menu: &NativeMenuModel,
    target: &mut i32,
    key: &str,
    min: i32,
    max: i32,
) -> bool {
    if let Some(value) = menu.number_for_key(key) {
        let value = value.clamp(min, max);
        if *target != value {
            *target = value;
            return true;
        }
    }
    false
}

fn apply_value_lane_menu_state(
    menu: &NativeMenuModel,
    lane: &mut NativeValueLane,
    prefix: &str,
) -> bool {
    let mut changed = false;
    changed |= set_bool_from_menu(menu, &mut lane.enabled, &format!("{prefix}.enabled"));
    changed |= set_u8_from_menu(menu, &mut lane.from, &format!("{prefix}.from"), 127);
    changed |= set_u8_from_menu(menu, &mut lane.to, &format!("{prefix}.to"), 127);
    changed |= set_i32_from_menu(
        menu,
        &mut lane.grid_offset,
        &format!("{prefix}.gridOffset"),
        -7,
        7,
    );
    changed |= set_string_from_menu(menu, &mut lane.curve, &format!("{prefix}.curve"));
    changed
}

fn apply_fx_param_menu_state(menu: &NativeMenuModel, params: &mut Value, prefix: &str) -> bool {
    let before = params.clone();
    let mut map = params.as_object().cloned().unwrap_or_default();
    if let Some(source) = menu.value_for_key(&format!("{prefix}.source")) {
        map.insert("source".into(), json!(source));
    }
    for (key, scale) in [
        ("threshold", 100.0),
        ("amountPct", 1.0),
        ("attackMs", 1.0),
        ("releaseMs", 1.0),
        ("mixPct", 1.0),
        ("timeMs", 1.0),
        ("feedback", 100.0),
        ("rateHz", 100.0),
        ("depthPct", 1.0),
        ("drive", 10.0),
        ("clip", 100.0),
        ("bits", 1.0),
        ("rateDiv", 1.0),
        ("depthMs", 10.0),
        ("baseMs", 10.0),
        ("centerHz", 1.0),
        ("q", 100.0),
        ("decay", 1000.0),
        ("damp", 100.0),
        ("chancePct", 1.0),
        ("sliceMs", 1.0),
        ("thresholdDb", 2.0),
        ("ratio", 2.0),
        ("makeupDb", 2.0),
        ("lowGainDb", 2.0),
        ("midGainDb", 2.0),
        ("highGainDb", 2.0),
        ("midFreqHz", 1.0),
        ("midQ", 100.0),
        ("saturationPct", 1.0),
        ("cracklePct", 1.0),
        ("warpDepthPct", 1.0),
    ] {
        if let Some(value) = menu.number_for_key(&format!("{prefix}.{key}")) {
            if scale == 1.0 {
                map.insert(key.into(), json!(value));
            } else {
                map.insert(key.into(), json!(f64::from(value) / scale));
            }
        }
    }
    *params = Value::Object(map);
    *params != before
}

fn set_u8_enum_from_menu(menu: &NativeMenuModel, target: &mut u8, key: &str, max: u8) -> bool {
    if let Some(value) = menu
        .value_for_key(key)
        .and_then(|value| value.parse::<u8>().ok())
    {
        let value = value.clamp(1, max);
        if *target != value {
            *target = value;
            return true;
        }
    }
    false
}

fn wrap_help_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let next_len = if current.is_empty() {
            word.len()
        } else {
            current.len() + 1 + word.len()
        };
        if next_len > width && !current.is_empty() {
            lines.push(current);
            current = word.to_string();
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push("No help available.".into());
    }
    lines
}

#[cfg(test)]
mod tests;
