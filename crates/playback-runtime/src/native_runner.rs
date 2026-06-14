use crate::native_menu::{
    NativeAuxBindingConfig, NativeFxBusConfig, NativeMenuAction, NativeMenuConfig, NativeMenuModel,
    NativeParamBindingSpec, NativeParamModsConfig, NativeSampleBrowserConfig,
    NativeSampleEntryConfig, NativeSensePartConfig, NativeValueLaneConfig,
};
use crate::protocol::{
    HostMessage, MidiPort, RunnerMessage, RuntimeAudioCommand, RuntimeMomentaryFxTarget,
    RuntimePlatformEffect, RuntimeStatus, RuntimeStatusState, RuntimeStoreResult,
    RuntimeTransportState, SampleEntry, SyncSource,
};
use crate::runtime::{CoreRunner, RuntimeConfig};
use platform_core::{
    default_mapping_config, AxisStrategy, BehaviorActionInput, BehaviorConfigItem,
    BehaviorConfigItemType, CellTriggerIntent, DeviceInput, GlobalSoundConfig, GridInteraction,
    InterpretationEventProfile, InterpretationProfile, InterpretationStateProfile, MusicalEvent,
    NativeBehavior, NativePartEngine, NativePartEngineConfig, NoteBehavior, RangeMode,
    TickStrategy, TriggerAction, TriggerTarget, VelocityCurve,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const GRID_WIDTH: usize = 8;
const GRID_HEIGHT: usize = 8;
const INSTRUMENT_SLOT_COUNT: usize = 8;
const PAN_POSITION_COUNT: u8 = 33;
const DEFAULT_ALGORITHM_STEP_PULSES: u32 = 12;
const OLED_BODY_ROWS: usize = 7;

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
                note_length_ms: 150,
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
    sample_velocity_levels_enabled: bool,
    sample_velocity_high: u8,
    sample_velocity_medium: u8,
    sample_velocity_low: u8,
    midi_enabled: bool,
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
    slot2_type: String,
    pan_pos: u8,
    auto_name: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NativeHelpPopup {
    title: String,
    lines: Vec<String>,
    scroll: usize,
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
            sample_paths: vec![None; 8],
            sample_assignments: Vec::new(),
            synth_config,
            synth_gain_pct: 80,
            sample_tune_semis: 0,
            sample_gain_pct: 100,
            sample_amp_env,
            sample_filter,
            sample_filter_env,
            sample_base_velocity: 100,
            sample_velocity_levels_enabled: false,
            sample_velocity_high: 120,
            sample_velocity_medium: 85,
            sample_velocity_low: 45,
            midi_enabled: false,
            midi_velocity: 100,
            midi_duration_ms: 120,
        }
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
        }
    }

    fn filter_cutoff_default() -> Self {
        Self {
            enabled: false,
            from: 20,
            to: 127,
            grid_offset: 0,
        }
    }

    fn filter_resonance_default() -> Self {
        Self {
            enabled: false,
            from: 10,
            to: 90,
            grid_offset: 0,
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
            slot2_type: "none".into(),
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
            master_volume: 100,
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
    midi_enabled: bool,
    preset_names: Vec<String>,
    current_preset_name: Option<String>,
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
    sample_browser: Option<NativeSampleBrowser>,
    help_popup: Option<NativeHelpPopup>,
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
        let instruments = default_instruments();
        let sense_parts = default_sense_parts();
        let fx_buses = default_fx_buses();
        let global_fx_slots = default_global_fx_slots();
        let menu = NativeMenuModel::new(NativeMenuConfig {
            behavior_id: behavior.id().into(),
            behavior_ids: platform_core::list_native_behavior_ids()
                .iter()
                .map(|id| (*id).to_string())
                .collect(),
            l1_items: vec![],
            part_labels: (0..GRID_HEIGHT)
                .map(|index| format!("P{}: life", index + 1))
                .collect(),
            part_names: vec![behavior.id().into(); GRID_HEIGHT],
            part_auto_names: vec![true; GRID_HEIGHT],
            sense_parts: sense_part_configs(&sense_parts),
            active_part_index: 0,
            param_mods: vec![NativeParamModsConfig::default(); GRID_HEIGHT],
            xy_x_binding: None,
            xy_y_binding: None,
            aux_bindings: vec![NativeAuxBindingConfig::default(); 4],
            instrument_labels: instrument_labels(&instruments),
            instrument_names: instrument_names(&instruments),
            instrument_types: instrument_types(&instruments),
            instrument_auto_names: instrument_auto_names(&instruments),
            instrument_routes: instrument_routes(&instruments),
            instrument_volumes: instrument_volumes(&instruments),
            instrument_pan_positions: instrument_pan_positions(&instruments),
            instrument_sample_slots: instrument_sample_slots(&instruments),
            instrument_synth_gain_pct: instrument_synth_gain_pct(&instruments),
            instrument_synth_filter_resonance: instrument_synth_filter_resonance(&instruments),
            instrument_sample_tune_semis: instrument_sample_tune_semis(&instruments),
            instrument_sample_gain_pct: instrument_sample_gain_pct(&instruments),
            instrument_midi_enabled: instrument_midi_enabled(&instruments),
            instrument_midi_velocity: instrument_midi_velocity(&instruments),
            instrument_midi_duration_ms: instrument_midi_duration_ms(&instruments),
            fx_buses: fx_bus_configs(&fx_buses),
            global_fx_slots: global_fx_slots.clone(),
            sample_browser: None,
            algorithm_step_pulses: DEFAULT_ALGORITHM_STEP_PULSES,
            master_volume: ui.master_volume,
            note_length_ms: config.global_sound.note_length_ms as u16,
            velocity_scale_pct: config.global_sound.velocity_scale_pct,
            velocity_curve: velocity_curve_id(config.global_sound.velocity_curve).into(),
            voice_stealing_mode: "balanced".into(),
            auto_save_default: true,
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
        part_engines.resize_with(GRID_HEIGHT, || None);
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
            part_behavior_configs: vec![config.behavior_config; GRID_HEIGHT],
            interpretation_profile: config.interpretation_profile,
            mapping_config: config.mapping_config,
            global_sound: config.global_sound,
            note_behaviors: config.note_behaviors,
            current_ppqn_pulse: 0,
            tick: 0,
            algorithm_step_pulses: DEFAULT_ALGORITHM_STEP_PULSES,
            algorithm_pulse_accumulator: 0,
            part_algorithm_step_pulses: vec![DEFAULT_ALGORITHM_STEP_PULSES; GRID_HEIGHT],
            part_pulse_accumulators: vec![0; GRID_HEIGHT],
            transport: RuntimeTransportState::Stopped,
            sync_source: config.sync_source,
            pending_resync: false,
            bpm: config.bpm,
            ui,
            midi_enabled: false,
            preset_names: Vec::new(),
            current_preset_name: None,
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
            param_mods: vec![NativeParamMods::default(); GRID_HEIGHT],
            trigger_gate_modes: vec!["full".into(); GRID_HEIGHT],
            trigger_gate_restore_modes: vec![None; GRID_HEIGHT],
            trigger_probability_assign: None,
            trigger_probability_maps: vec![
                vec!["full".into(); GRID_WIDTH * GRID_HEIGHT];
                GRID_HEIGHT
            ],
            part_behavior_ids: vec![behavior.id().into(); GRID_HEIGHT],
            part_names: vec![behavior.id().into(); GRID_HEIGHT],
            part_auto_names: vec![true; GRID_HEIGHT],
            save_grid_states: vec![true; GRID_HEIGHT],
            sense_parts,
            aux_bindings: vec![None; 4],
            active_part_index: 0,
            instruments,
            sample_assign: None,
            fx_buses,
            global_fx_slots,
            sample_browser: None,
            help_popup: None,
            menu,
            event_dot_on: false,
            event_dot_pulses_remaining: 0,
            transport_flash: "none",
            transport_flash_pulses_remaining: 0,
            auto_save_default: true,
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

    fn menu_config(&self) -> NativeMenuConfig {
        NativeMenuConfig {
            behavior_id: self.behavior.id().into(),
            behavior_ids: platform_core::list_native_behavior_ids()
                .iter()
                .map(|id| (*id).to_string())
                .collect(),
            l1_items: self.l1_menu_items(),
            part_labels: self.part_labels(),
            part_names: self.part_names.clone(),
            part_auto_names: self.part_auto_names.clone(),
            sense_parts: sense_part_configs(&self.sense_parts),
            active_part_index: self.active_part_index,
            param_mods: param_mod_configs(&self.param_mods),
            xy_x_binding: self
                .xy_x_binding
                .as_ref()
                .map(param_binding_spec_from_native),
            xy_y_binding: self
                .xy_y_binding
                .as_ref()
                .map(param_binding_spec_from_native),
            aux_bindings: aux_binding_configs(&self.aux_bindings),
            instrument_labels: instrument_labels(&self.instruments),
            instrument_names: instrument_names(&self.instruments),
            instrument_types: instrument_types(&self.instruments),
            instrument_auto_names: instrument_auto_names(&self.instruments),
            instrument_routes: instrument_routes(&self.instruments),
            instrument_volumes: instrument_volumes(&self.instruments),
            instrument_pan_positions: instrument_pan_positions(&self.instruments),
            instrument_sample_slots: instrument_sample_slots(&self.instruments),
            instrument_synth_gain_pct: instrument_synth_gain_pct(&self.instruments),
            instrument_synth_filter_resonance: instrument_synth_filter_resonance(&self.instruments),
            instrument_sample_tune_semis: instrument_sample_tune_semis(&self.instruments),
            instrument_sample_gain_pct: instrument_sample_gain_pct(&self.instruments),
            instrument_midi_enabled: instrument_midi_enabled(&self.instruments),
            instrument_midi_velocity: instrument_midi_velocity(&self.instruments),
            instrument_midi_duration_ms: instrument_midi_duration_ms(&self.instruments),
            fx_buses: fx_bus_configs(&self.fx_buses),
            global_fx_slots: self.global_fx_slots.clone(),
            sample_browser: self
                .sample_browser
                .as_ref()
                .map(|browser| NativeSampleBrowserConfig {
                    instrument_slot: browser.instrument_slot,
                    sample_slot: browser.sample_slot,
                    dir: browser.dir.clone(),
                    entries: browser
                        .entries
                        .iter()
                        .map(|entry| NativeSampleEntryConfig {
                            name: entry.name.clone(),
                            path: entry.path.clone(),
                            is_dir: entry.is_dir,
                        })
                        .collect(),
                }),
            algorithm_step_pulses: self.algorithm_step_pulses,
            master_volume: self.ui.master_volume,
            note_length_ms: self.global_sound.note_length_ms as u16,
            velocity_scale_pct: self.global_sound.velocity_scale_pct,
            velocity_curve: velocity_curve_id(self.global_sound.velocity_curve).into(),
            voice_stealing_mode: self.voice_stealing_mode.clone(),
            auto_save_default: self.auto_save_default,
            ghost_cells: self.ui.ghost_cells,
            input_events_while_paused: self.input_events_while_paused,
            numeric_display_mode: self.ui.numeric_display_mode.clone(),
            screen_sleep_seconds: self.ui.screen_sleep_seconds,
            grid_brightness: self.ui.grid_brightness,
            display_brightness: self.ui.display_brightness,
            button_brightness: self.ui.button_brightness,
            midi_enabled: self.midi_enabled,
            midi_clock_out_enabled: self.midi_clock_out_enabled,
            midi_clock_in_enabled: self.midi_clock_in_enabled,
            midi_respond_to_start_stop: self.midi_respond_to_start_stop,
            preset_names: self.preset_names.clone(),
            midi_outputs: self
                .midi_outputs
                .iter()
                .map(|port| (port.id.clone(), port.name.clone()))
                .collect(),
            midi_inputs: self
                .midi_inputs
                .iter()
                .map(|port| (port.id.clone(), port.name.clone()))
                .collect(),
            dance_mode: self.dance_mode.clone(),
            dance_fx_type: dance_fx_type(&self.dance_fx_selected).into(),
            dance_fx_target: dance_fx_target_key(&self.dance_fx_selected).into(),
            dance_fx_params: dance_fx_params_map(&self.dance_fx_selected),
            xy_release: self.xy_release.clone(),
            xy_invert_x: self.xy_invert_x,
            xy_invert_y: self.xy_invert_y,
            bpm: self.bpm.round().clamp(20.0, 300.0) as u16,
            sync_source: self.sync_source.clone(),
        }
    }

    fn apply_menu_state(&mut self) -> Result<(), String> {
        let before_payload = self.config_payload();
        if let Some(sync_source) = self.menu.selected_sync_source() {
            self.sync_source = sync_source;
        }
        if let Some(step_pulses) = self.menu.selected_algorithm_step_pulses() {
            self.algorithm_step_pulses = step_pulses;
            if let Some(part_step) = self
                .part_algorithm_step_pulses
                .get_mut(self.active_part_index)
            {
                *part_step = step_pulses;
            }
        }
        if let Some(master_volume) = self.menu.selected_master_volume() {
            self.ui.master_volume = master_volume;
        }
        if let Some(midi_enabled) = self
            .menu
            .value_for_key("midiEnabled")
            .map(|value| value == "true")
        {
            self.midi_enabled = midi_enabled;
        }
        if let Some(clock_out_enabled) = self
            .menu
            .value_for_key("midi.clockOutEnabled")
            .map(|value| value == "true")
        {
            self.midi_clock_out_enabled = clock_out_enabled;
        }
        if let Some(clock_in_enabled) = self
            .menu
            .value_for_key("midi.clockInEnabled")
            .map(|value| value == "true")
        {
            self.midi_clock_in_enabled = clock_in_enabled;
        }
        if let Some(respond_to_start_stop) = self
            .menu
            .value_for_key("midi.respondToStartStop")
            .map(|value| value == "true")
        {
            self.midi_respond_to_start_stop = respond_to_start_stop;
        }
        if let Some(dance_mode) = self.menu.selected_dance_mode() {
            let changed = self.dance_mode != dance_mode;
            self.dance_mode = dance_mode.clone();
            if changed && self.menu.state.stack.first() == Some(&3) {
                self.active_dance_mode = dance_mode;
            }
        }
        let dance_fx_changed = self.apply_dance_fx_menu_state();
        if let Some(xy_release) = self.menu.value_for_key("dance.xy.release") {
            self.xy_release = xy_release;
        }
        if let Some(invert_x) = self.menu.value_for_key("dance.xy.invertX") {
            self.xy_invert_x = invert_x == "true";
        }
        if let Some(invert_y) = self.menu.value_for_key("dance.xy.invertY") {
            self.xy_invert_y = invert_y == "true";
        }
        if let Some(display_brightness) = self.menu.selected_display_brightness() {
            self.ui.display_brightness = display_brightness;
        }
        if let Some(button_brightness) = self.menu.selected_button_brightness() {
            self.ui.button_brightness = button_brightness;
        }
        if let Some(bpm) = self.menu.number_for_key("transport.bpm") {
            self.bpm = f64::from(bpm.clamp(40, 240));
        }
        if let Some(note_length_ms) = self.menu.number_for_key("sound.noteLengthMs") {
            self.global_sound.note_length_ms = note_length_ms.clamp(30, 2000) as u32;
        }
        if let Some(velocity_scale_pct) = self.menu.number_for_key("sound.velocityScalePct") {
            self.global_sound.velocity_scale_pct = velocity_scale_pct.clamp(0, 200) as u16;
        }
        if let Some(velocity_curve) = self.menu.value_for_key("sound.velocityCurve") {
            self.global_sound.velocity_curve = velocity_curve_from_id(&velocity_curve);
        }
        if let Some(voice_stealing_mode) = self.menu.value_for_key("sound.voiceStealingMode") {
            if matches!(
                voice_stealing_mode.as_str(),
                "off" | "lenient" | "balanced" | "aggressive"
            ) {
                self.voice_stealing_mode = voice_stealing_mode;
            }
        }
        if let Some(ghost_cells) = self.menu.value_for_key("ghostCells") {
            self.ui.ghost_cells = ghost_cells == "true";
        }
        if let Some(input_events_while_paused) = self.menu.value_for_key("inputEventsWhilePaused") {
            self.input_events_while_paused = input_events_while_paused == "true";
        }
        if let Some(numeric_display_mode) = self.menu.value_for_key("numericDisplayMode") {
            self.ui.numeric_display_mode = numeric_display_mode;
        }
        if let Some(screen_sleep_seconds) = self.menu.number_for_key("screenSleepSeconds") {
            self.ui.screen_sleep_seconds = screen_sleep_seconds.clamp(0, 600) as u16;
        }
        if let Some(grid_brightness) = self.menu.number_for_key("gridBrightness") {
            self.ui.grid_brightness = grid_brightness.clamp(10, 100) as u8;
        }
        self.apply_param_mod_invert_menu_state();
        let part_changed = self.apply_part_menu_state();
        let instrument_changed = self.apply_instrument_menu_state();
        let sense_changed = self.apply_sense_menu_state();
        let fx_changed = self.apply_fx_menu_state();
        if part_changed || instrument_changed || sense_changed || fx_changed || dance_fx_changed {
            self.menu.rebuild(self.menu_config());
        }
        if sense_changed {
            self.refresh_active_interpretation_profile();
            self.engine
                .set_interpretation_profile(self.interpretation_profile.clone());
        }
        if let Some(behavior_id) = self.menu.selected_behavior().map(|value| value.to_string()) {
            if behavior_id.as_str() != self.behavior.id() {
                let previous_behavior_id = self.behavior.id().to_string();
                self.behavior_configs
                    .insert(self.behavior.id().to_string(), self.behavior_config.clone());
                if let Some(config) = self.part_behavior_configs.get_mut(self.active_part_index) {
                    *config = self.behavior_config.clone();
                }
                let behavior = platform_core::get_native_behavior(&behavior_id)
                    .ok_or_else(|| format!("unsupported native behavior `{behavior_id}`"))?;
                self.behavior_config = self
                    .part_behavior_configs
                    .get(self.active_part_index)
                    .filter(|config| !config.is_null())
                    .cloned()
                    .or_else(|| self.behavior_configs.get(&behavior_id).cloned())
                    .unwrap_or(Value::Null);
                self.behavior_configs
                    .insert(behavior_id.clone(), self.behavior_config.clone());
                if let Some(config) = self.part_behavior_configs.get_mut(self.active_part_index) {
                    *config = self.behavior_config.clone();
                }
                if let Some(part_behavior_id) =
                    self.part_behavior_ids.get_mut(self.active_part_index)
                {
                    *part_behavior_id = behavior_id.clone();
                }
                if self
                    .part_auto_names
                    .get(self.active_part_index)
                    .copied()
                    .unwrap_or(true)
                {
                    if let Some(name) = self.part_names.get_mut(self.active_part_index) {
                        *name = behavior_id.clone();
                    }
                }
                self.remap_bindings_for_behavior_change(
                    &previous_behavior_id,
                    &behavior_id,
                    self.active_part_index,
                );
                self.rebuild_engine(behavior)?;
            }
        }
        let next_behavior_config = self.behavior_config_from_menu()?;
        if next_behavior_config != self.behavior_config {
            self.behavior_config = next_behavior_config;
            if let Some(config) = self.part_behavior_configs.get_mut(self.active_part_index) {
                *config = self.behavior_config.clone();
            }
            self.behavior_configs
                .insert(self.behavior.id().to_string(), self.behavior_config.clone());
            self.rebuild_engine(self.behavior)?;
        }
        self.refresh_active_mapping_config();
        self.refresh_active_interpretation_profile();
        self.engine
            .set_interpretation_profile(self.interpretation_profile.clone());
        self.auto_save_default = self
            .menu
            .value_for_key("autoSaveDefault")
            .map(|value| value == "true")
            .unwrap_or(self.auto_save_default);
        if self.config_payload() != before_payload {
            self.config_dirty = true;
        }
        Ok(())
    }

    fn apply_param_mod_invert_menu_state(&mut self) {
        for part_index in 0..self.param_mods.len() {
            for axis in ["x", "y"] {
                for slot in 0..2 {
                    let key = format!("parts.{part_index}.paramMods.{axis}.{slot}.invert");
                    let Some(value) = self.menu.value_for_key(&key) else {
                        continue;
                    };
                    let invert = value == "true";
                    let target = if axis == "x" {
                        self.param_mods[part_index].x.get_mut(slot)
                    } else {
                        self.param_mods[part_index].y.get_mut(slot)
                    };
                    if let Some(Some(binding)) = target {
                        if binding.invert != invert {
                            binding.invert = invert;
                            self.config_dirty = true;
                        }
                    }
                }
            }
        }
    }

    fn refresh_active_mapping_config(&mut self) {
        let mapping = self.mapping_config_for_part(self.active_part_index);
        self.engine.set_mapping_config(mapping.clone());
        self.mapping_config = mapping;
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

    fn refresh_active_interpretation_profile(&mut self) {
        self.interpretation_profile = self.interpretation_profile_for_part(self.active_part_index);
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

    fn apply_part_menu_state(&mut self) -> bool {
        let mut changed = false;
        for index in 0..self.part_auto_names.len() {
            let before_name = self.part_names.get(index).cloned().unwrap_or_default();
            let Some(auto_name) = self
                .menu
                .value_for_key(&format!("parts.{index}.autoName"))
                .map(|value| value == "true")
            else {
                continue;
            };
            if self.part_auto_names[index] != auto_name {
                self.part_auto_names[index] = auto_name;
                if auto_name {
                    let behavior_id = self
                        .part_behavior_ids
                        .get(index)
                        .cloned()
                        .unwrap_or_else(|| self.behavior.id().into());
                    if let Some(name) = self.part_names.get_mut(index) {
                        *name = behavior_id;
                    }
                }
                changed = true;
            }
            if let Some(name) = self.menu.value_for_key(&format!("parts.{index}.name")) {
                if name != before_name {
                    if let Some(target) = self.part_names.get_mut(index) {
                        *target = name;
                    }
                    if let Some(auto_name) = self.part_auto_names.get_mut(index) {
                        *auto_name = false;
                    }
                    changed = true;
                }
            }
        }
        changed
    }

    fn apply_instrument_menu_state(&mut self) -> bool {
        let mut changed = false;
        for index in 0..self.instruments.len() {
            let Some(instrument) = self.instruments.get_mut(index) else {
                continue;
            };
            let before_name = instrument.name.clone();
            if let Some(kind) = self
                .menu
                .value_for_key(&format!("instruments.{index}.type"))
            {
                if instrument.kind != kind {
                    instrument.kind = kind;
                    if instrument.auto_name {
                        instrument.name = derive_instrument_name(index, &instrument.kind);
                    }
                    changed = true;
                }
            }
            if let Some(note_behavior) = self
                .menu
                .value_for_key(&format!("instruments.{index}.noteBehavior"))
            {
                if instrument.note_behavior != note_behavior {
                    instrument.note_behavior = note_behavior;
                    changed = true;
                }
            }
            if let Some(auto_name) = self
                .menu
                .value_for_key(&format!("instruments.{index}.autoName"))
                .map(|value| value == "true")
            {
                if instrument.auto_name != auto_name {
                    instrument.auto_name = auto_name;
                    if auto_name {
                        instrument.name = derive_instrument_name(index, &instrument.kind);
                    }
                    changed = true;
                }
            }
            if let Some(name) = self
                .menu
                .value_for_key(&format!("instruments.{index}.name"))
            {
                if name != before_name {
                    instrument.name = name;
                    instrument.auto_name = false;
                    changed = true;
                }
            }
            if let Some(volume) = self
                .menu
                .number_for_key(&format!("instruments.{index}.mixer.volume"))
            {
                let volume = volume.clamp(0, 100) as u8;
                if instrument.volume != volume {
                    instrument.volume = volume;
                    changed = true;
                }
            }
            if let Some(pan_pos) = self
                .menu
                .number_for_key(&format!("instruments.{index}.mixer.panPos"))
            {
                let pan_pos = pan_pos.clamp(0, i32::from(PAN_POSITION_COUNT - 1)) as u8;
                if instrument.pan_pos != pan_pos {
                    instrument.pan_pos = pan_pos;
                    changed = true;
                }
            }
            if let Some(route) = self
                .menu
                .value_for_key(&format!("instruments.{index}.mixer.route"))
            {
                if instrument.route != route {
                    instrument.route = route;
                    changed = true;
                }
            }
            if let Some(sample_slot) = self
                .menu
                .value_for_key(&format!("instruments.{index}.sample.selectedSlot"))
                .and_then(|value| value.parse::<usize>().ok())
                .and_then(|value| value.checked_sub(1))
            {
                let sample_slot = sample_slot.min(7);
                if instrument.selected_sample_slot != sample_slot {
                    instrument.selected_sample_slot = sample_slot;
                    changed = true;
                }
            }
            if let Some(gain) = self
                .menu
                .number_for_key(&format!("instruments.{index}.synth.amp.gainPct"))
            {
                let gain = gain.clamp(0, 100) as u8;
                if instrument.synth_gain_pct != gain {
                    instrument.synth_gain_pct = gain;
                    set_json_path_number(
                        &mut instrument.synth_config,
                        &["amp", "gainPct"],
                        f64::from(gain),
                    );
                    changed = true;
                }
            }
            if let Some(resonance) = self
                .menu
                .number_for_key(&format!("instruments.{index}.synth.filter.resonance"))
            {
                let resonance = resonance.clamp(0, 255) as u8;
                if synth_filter_resonance(instrument) != resonance {
                    set_json_path_number(
                        &mut instrument.synth_config,
                        &["filter", "resonance"],
                        f64::from(resonance),
                    );
                    changed = true;
                }
            }
            if let Some(tune) = self
                .menu
                .number_for_key(&format!("instruments.{index}.sample.tuneSemis"))
            {
                let tune = tune.clamp(-24, 24) as i8;
                if instrument.sample_tune_semis != tune {
                    instrument.sample_tune_semis = tune;
                    changed = true;
                }
            }
            if let Some(gain) = self
                .menu
                .number_for_key(&format!("instruments.{index}.sample.amp.gainPct"))
            {
                let gain = gain.clamp(0, 100) as u8;
                if instrument.sample_gain_pct != gain {
                    instrument.sample_gain_pct = gain;
                    changed = true;
                }
            }
            if let Some(enabled) = self
                .menu
                .value_for_key(&format!("instruments.{index}.midi.enabled"))
                .map(|value| value == "true")
            {
                if instrument.midi_enabled != enabled {
                    instrument.midi_enabled = enabled;
                    changed = true;
                }
            }
            if let Some(velocity) = self
                .menu
                .number_for_key(&format!("instruments.{index}.midi.velocity"))
            {
                let velocity = velocity.clamp(1, 127) as u8;
                if instrument.midi_velocity != velocity {
                    instrument.midi_velocity = velocity;
                    changed = true;
                }
            }
            if let Some(duration_ms) = self
                .menu
                .number_for_key(&format!("instruments.{index}.midi.durationMs"))
            {
                let duration_ms = duration_ms.clamp(10, 2000) as u16;
                if instrument.midi_duration_ms != duration_ms {
                    instrument.midi_duration_ms = duration_ms;
                    changed = true;
                }
            }
        }
        changed
    }

    fn apply_sense_menu_state(&mut self) -> bool {
        let mut changed = false;
        for index in 0..self.sense_parts.len() {
            let prefix = format!("parts.{index}.l2");
            let Some(part) = self.sense_parts.get_mut(index) else {
                continue;
            };
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.scan_mode,
                &format!("{prefix}.scanMode"),
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.scan_axis,
                &format!("{prefix}.scanAxis"),
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.scan_unit,
                &format!("{prefix}.scanUnit"),
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.scan_direction,
                &format!("{prefix}.scanDirection"),
            );
            changed |= set_u8_enum_from_menu(
                &self.menu,
                &mut part.scan_sections,
                &format!("{prefix}.scanSections"),
                8,
            );
            changed |= set_target_slot_from_menu(
                &self.menu,
                &mut part.scanned_slot,
                &format!("{prefix}.mapping.scanned.slot"),
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.scanned_action,
                &format!("{prefix}.mapping.scanned.action"),
            );
            changed |= set_target_slot_from_menu(
                &self.menu,
                &mut part.scanned_empty_slot,
                &format!("{prefix}.mapping.scanned_empty.slot"),
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.scanned_empty_action,
                &format!("{prefix}.mapping.scanned_empty.action"),
            );
            changed |= set_bool_from_menu(
                &self.menu,
                &mut part.event_enabled,
                &format!("{prefix}.eventEnabled"),
            );
            changed |= set_bool_from_menu(
                &self.menu,
                &mut part.state_notes_enabled,
                &format!("{prefix}.stateNotesEnabled"),
            );
            changed |= set_target_slot_from_menu(
                &self.menu,
                &mut part.activate_slot,
                &format!("{prefix}.mapping.activate.slot"),
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.activate_action,
                &format!("{prefix}.mapping.activate.action"),
            );
            changed |= set_target_slot_from_menu(
                &self.menu,
                &mut part.stable_slot,
                &format!("{prefix}.mapping.stable.slot"),
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.stable_action,
                &format!("{prefix}.mapping.stable.action"),
            );
            changed |= set_target_slot_from_menu(
                &self.menu,
                &mut part.deactivate_slot,
                &format!("{prefix}.mapping.deactivate.slot"),
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.deactivate_action,
                &format!("{prefix}.mapping.deactivate.action"),
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.trigger_probability_mode,
                &format!("{prefix}.triggerProbabilityMode"),
            );
            changed |= set_u8_from_menu(
                &self.menu,
                &mut part.trigger_probability_low_pct,
                &format!("{prefix}.triggerProbabilityLowPct"),
                100,
            );
            changed |= set_u8_from_menu(
                &self.menu,
                &mut part.trigger_probability_high_pct,
                &format!("{prefix}.triggerProbabilityHighPct"),
                100,
            );
            changed |= set_u8_from_menu(
                &self.menu,
                &mut part.lowest_note,
                &format!("{prefix}.pitch.lowestNote"),
                127,
            );
            changed |= set_u8_from_menu(
                &self.menu,
                &mut part.highest_note,
                &format!("{prefix}.pitch.highestNote"),
                127,
            );
            changed |= set_u8_from_menu(
                &self.menu,
                &mut part.starting_note,
                &format!("{prefix}.pitch.startingNote"),
                127,
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.scale,
                &format!("{prefix}.pitch.scale"),
            );
            changed |=
                set_string_from_menu(&self.menu, &mut part.root, &format!("{prefix}.pitch.root"));
            changed |= set_string_from_menu(
                &self.menu,
                &mut part.out_of_range,
                &format!("{prefix}.pitch.outOfRange"),
            );
            changed |= set_bool_from_menu(
                &self.menu,
                &mut part.x_pitch_enabled,
                &format!("{prefix}.x.pitch.enabled"),
            );
            changed |= set_i32_from_menu(
                &self.menu,
                &mut part.x_pitch_steps,
                &format!("{prefix}.x.pitch.steps"),
                -16,
                16,
            );
            changed |= set_bool_from_menu(
                &self.menu,
                &mut part.x_pitch_restart_each_section,
                &format!("{prefix}.x.pitch.restartEachSection"),
            );
            changed |= set_bool_from_menu(
                &self.menu,
                &mut part.y_pitch_enabled,
                &format!("{prefix}.y.pitch.enabled"),
            );
            changed |= set_i32_from_menu(
                &self.menu,
                &mut part.y_pitch_steps,
                &format!("{prefix}.y.pitch.steps"),
                -16,
                16,
            );
            changed |= set_bool_from_menu(
                &self.menu,
                &mut part.y_pitch_restart_each_section,
                &format!("{prefix}.y.pitch.restartEachSection"),
            );
            changed |= apply_value_lane_menu_state(
                &self.menu,
                &mut part.x_velocity,
                &format!("{prefix}.x.velocity"),
            );
            changed |= apply_value_lane_menu_state(
                &self.menu,
                &mut part.x_filter_cutoff,
                &format!("{prefix}.x.filterCutoff"),
            );
            changed |= apply_value_lane_menu_state(
                &self.menu,
                &mut part.x_filter_resonance,
                &format!("{prefix}.x.filterResonance"),
            );
            changed |= apply_value_lane_menu_state(
                &self.menu,
                &mut part.y_velocity,
                &format!("{prefix}.y.velocity"),
            );
            changed |= apply_value_lane_menu_state(
                &self.menu,
                &mut part.y_filter_cutoff,
                &format!("{prefix}.y.filterCutoff"),
            );
            changed |= apply_value_lane_menu_state(
                &self.menu,
                &mut part.y_filter_resonance,
                &format!("{prefix}.y.filterResonance"),
            );
        }
        changed
    }

    fn apply_fx_menu_state(&mut self) -> bool {
        let mut changed = false;
        for index in 0..self.fx_buses.len() {
            let prefix = format!("mixer.buses.{index}");
            let Some(bus) = self.fx_buses.get_mut(index) else {
                continue;
            };
            let before = (
                bus.slot1_type.clone(),
                bus.slot2_type.clone(),
                bus.auto_name,
                bus.name.clone(),
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut bus.slot1_type,
                &format!("{prefix}.slot1.type"),
            );
            changed |= set_string_from_menu(
                &self.menu,
                &mut bus.slot2_type,
                &format!("{prefix}.slot2.type"),
            );
            changed |= set_u8_from_menu(
                &self.menu,
                &mut bus.pan_pos,
                &format!("{prefix}.panPos"),
                32,
            );
            changed |= set_bool_from_menu(
                &self.menu,
                &mut bus.auto_name,
                &format!("{prefix}.autoName"),
            );
            if bus.auto_name
                && (bus.slot1_type != before.0 || bus.slot2_type != before.1 || !before.2)
            {
                bus.name = derive_bus_name(bus);
                changed = true;
            }
            if let Some(name) = self.menu.value_for_key(&format!("{prefix}.name")) {
                if name != before.3 {
                    bus.name = name;
                    bus.auto_name = false;
                    changed = true;
                }
            }
        }
        for index in 0..self.global_fx_slots.len() {
            let Some(slot) = self.global_fx_slots.get_mut(index) else {
                continue;
            };
            changed |= set_string_from_menu(
                &self.menu,
                slot,
                &format!("mixer.master.slots.{index}.type"),
            );
        }
        changed
    }

    fn apply_dance_fx_menu_state(&mut self) -> bool {
        let before = self.dance_fx_selected.clone();
        let fx_type = self
            .menu
            .value_for_key("dance.fx.type")
            .unwrap_or_else(|| dance_fx_type(&before).into());
        let target = self
            .menu
            .value_for_key("dance.fx.target")
            .unwrap_or_else(|| dance_fx_target_key(&before).into());
        let mut params = serde_json::Map::new();
        for key in dance_fx_param_keys(&fx_type) {
            let default = dance_fx_param_default(&fx_type, key);
            let value = self
                .menu
                .number_for_key(&format!("dance.fx.params.{key}"))
                .unwrap_or_else(|| {
                    before
                        .get("params")
                        .and_then(|params| params.get(key))
                        .and_then(Value::as_i64)
                        .map(|value| value as i32)
                        .unwrap_or(default)
                });
            params.insert((*key).into(), Value::from(value));
        }
        self.dance_fx_selected = json!({
            "fxType": fx_type,
            "targetKey": target,
            "params": params,
        });
        self.dance_fx_selected != before
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
                label: "Name".into(),
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
        let key = format!("behavior.{}", item.key);
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
                let key = format!("behavior.{}", item.key);
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

    fn snapshot(&self) -> Result<Value, String> {
        let model = self.engine.model()?;
        let menu = self.menu.snapshot();
        let instruments = self
            .instruments
            .iter()
            .map(|instrument| {
                let sample_slots = instrument
                    .sample_paths
                    .iter()
                    .map(|path| json!({ "path": path }))
                    .collect::<Vec<_>>();
                json!({
                    "type": instrument.kind,
                    "noteBehavior": instrument.note_behavior,
                    "autoName": instrument.auto_name,
                    "name": instrument.name,
                    "synth": instrument.synth_config,
                    "sample": {
                        "selectedSlot": instrument.selected_sample_slot,
                        "slots": sample_slots,
                        "assignments": sample_assignments_payload(&instrument.sample_assignments),
                        "tuneSemis": instrument.sample_tune_semis,
                        "amp": { "gainPct": instrument.sample_gain_pct }
                    },
                    "midi": {
                        "enabled": instrument.midi_enabled,
                        "velocity": instrument.midi_velocity,
                        "durationMs": instrument.midi_duration_ms
                    },
                    "mixer": {
                        "volume": instrument.volume,
                        "panPos": instrument.pan_pos,
                        "route": instrument.route
                    }
                })
            })
            .collect::<Vec<_>>();
        let mut leds = vec![json!({ "r": 15, "g": 15, "b": 22 }); GRID_WIDTH * GRID_HEIGHT];
        for (logical_index, alive) in model.cells.iter().enumerate() {
            let x = logical_index % GRID_WIDTH;
            let y = logical_index / GRID_WIDTH;
            let display_index = display_index(x, y);
            let trigger = model
                .trigger_types
                .as_ref()
                .and_then(|types| types.get(logical_index))
                .copied();
            leds[display_index] = if !alive {
                json!({ "r": 15, "g": 15, "b": 22 })
            } else {
                match trigger.unwrap_or(platform_core::CellTriggerType::Stable) {
                    platform_core::CellTriggerType::Activate => {
                        json!({ "r": 255, "g": 255, "b": 255 })
                    }
                    platform_core::CellTriggerType::Deactivate => {
                        json!({ "r": 128, "g": 128, "b": 128 })
                    }
                    platform_core::CellTriggerType::Scanned => {
                        json!({ "r": 255, "g": 0, "b": 0 })
                    }
                    _ => json!({ "r": 0, "g": 255, "b": 120 }),
                }
            };
        }
        self.apply_scan_progress_overlay(&mut leds);
        self.apply_sample_assignment_overlay(&mut leds);
        self.apply_trigger_probability_overlay(&mut leds);
        self.apply_dance_overlay(&mut leds);
        self.apply_param_mod_overlay(&mut leds);
        self.apply_fn_overlay(&mut leds);

        let (
            display_lines,
            mut display_colors,
            mut display_bar_values,
            selected_row,
            display_title,
        ) = if let Some(help) = &self.help_popup {
            let mut lines = help
                .lines
                .iter()
                .skip(help.scroll)
                .take(OLED_BODY_ROWS - 1)
                .cloned()
                .collect::<Vec<_>>();
            lines.push("> Close".into());
            let colors = vec![0xFFFF; lines.len()];
            let bar_values = vec![Value::Null; lines.len()];
            (
                lines,
                colors,
                bar_values,
                Some(
                    help.lines
                        .len()
                        .saturating_sub(help.scroll)
                        .min(OLED_BODY_ROWS - 1),
                ),
                help.title.clone(),
            )
        } else {
            let bar_values = menu
                .bar_values
                .into_iter()
                .map(|bar| {
                    bar.map(|bar| {
                        json!({
                            "frac": f32::from(bar.frac_pct) / 100.0,
                            "numChars": bar.num_chars,
                            "style": bar.style,
                        })
                    })
                    .unwrap_or(Value::Null)
                })
                .collect::<Vec<_>>();
            (
                menu.lines,
                menu.colors,
                bar_values,
                menu.selected_row,
                menu.path,
            )
        };
        let display_title = clip_display_line(&display_title, 28);
        let display_lines = display_lines
            .into_iter()
            .take(OLED_BODY_ROWS)
            .map(|line| clip_display_line(&line, 28))
            .collect::<Vec<_>>();
        display_colors.truncate(display_lines.len());
        display_bar_values.truncate(display_lines.len());
        let toast = self.toast.as_ref().map(scrolled_toast).unwrap_or_default();

        Ok(json!({
            "display": {
                "page": self.behavior.id(),
                "title": display_title,
                "lines": display_lines,
                "colors": display_colors,
                "barValues": display_bar_values,
                "toast": toast,
                "editing": self.menu.state.editing && self.help_popup.is_none()
            },
            "leds": {
                "width": GRID_WIDTH,
                "height": GRID_HEIGHT,
                "cells": leds
            },
            "transport": {
                "playing": self.transport == RuntimeTransportState::Playing,
                "bpm": self.bpm,
                "tick": self.tick,
                "ppqnPulse": self.current_ppqn_pulse
            },
            "activeBehavior": self.behavior.id(),
            "danceMode": self.dance_mode,
            "activeDanceMode": self.active_dance_mode,
            "gridInteraction": match self.behavior.grid_interaction().unwrap_or(GridInteraction::Paint) {
                GridInteraction::Paint => "paint",
                GridInteraction::Momentary => "momentary",
            },
            "settings": {
                "displayBrightness": self.ui.display_brightness,
                "gridBrightness": self.ui.grid_brightness,
                "buttonBrightness": self.ui.button_brightness,
                "masterVolume": self.ui.master_volume,
                "sound": {
                    "noteLengthMs": self.global_sound.note_length_ms,
                    "velocityScalePct": self.global_sound.velocity_scale_pct,
                    "velocityCurve": velocity_curve_id(self.global_sound.velocity_curve),
                    "voiceStealingMode": self.voice_stealing_mode.clone()
                },
                "noteLengthMs": self.global_sound.note_length_ms,
                "velocityScalePct": self.global_sound.velocity_scale_pct,
                "velocityCurve": velocity_curve_id(self.global_sound.velocity_curve),
                "voiceStealingMode": self.voice_stealing_mode.clone(),
                "ghostCells": self.ui.ghost_cells,
                "inputEventsWhilePaused": self.input_events_while_paused,
                "numericDisplayMode": self.ui.numeric_display_mode,
                "screenSleepSeconds": self.ui.screen_sleep_seconds,
                "instruments": instruments,
                "mixer": self.mixer_payload(),
                "panPositions": PAN_POSITION_COUNT,
                "autoSaveFlash": if self.auto_save_flash_pulses_remaining > 0 { "flash" } else { "none" },
                "autoSaveFlashSerial": self.auto_save_flash_serial,
                "transportFlash": "none",
                "stopLatched": false,
                "fnHeld": self.ui.fn_held,
                "combinedModifierHeld": self.ui.combined_modifier_held,
                "midi": {
                    "enabled": self.midi_enabled,
                    "outId": self.selected_midi_output_id,
                    "inId": self.selected_midi_input_id,
                    "outputs": self.midi_outputs,
                    "inputs": self.midi_inputs,
                    "status": self.midi_status,
                    "syncMode": match self.sync_source {
                        SyncSource::Internal => "internal",
                        SyncSource::External => "external",
                    },
                    "clockOutEnabled": self.midi_clock_out_enabled,
                    "clockInEnabled": self.midi_clock_in_enabled,
                    "respondToStartStop": self.midi_respond_to_start_stop
                }
            },
            "selectedRow": selected_row,
            "voiceStealingMode": self.voice_stealing_mode.clone(),
            "eventDotOn": self.event_dot_on || self.event_dot_pulses_remaining > 0,
            "transportIcon": if self.transport == RuntimeTransportState::Playing { "play" } else { "stop" },
            "transportFlash": self.transport_flash,
            "cpuLoadRatio": 0.0
        }))
    }

    fn status(&self) -> RuntimeStatus {
        RuntimeStatus {
            state: RuntimeStatusState::Running,
            transport: self.transport.clone(),
            current_ppqn_pulse: self.current_ppqn_pulse,
            pending_resync: self.pending_resync,
            sync_source: self.sync_source.clone(),
            message: None,
        }
    }

    fn messages_with_snapshot(&mut self) -> Result<Vec<RunnerMessage>, String> {
        let snapshot = self.snapshot()?;
        let mut messages = vec![
            RunnerMessage::Snapshot { snapshot },
            RunnerMessage::RuntimeStatus {
                status: self.status(),
            },
        ];
        if self.auto_save_default && self.config_dirty {
            self.config_dirty = false;
            self.auto_save_flash_serial = self.auto_save_flash_serial.wrapping_add(1);
            self.auto_save_flash_pulses_remaining = 8;
            self.toast = Some(NativeToast {
                message: "Saved default".into(),
                offset: 0,
            });
            messages.insert(
                0,
                RunnerMessage::PlatformEffects {
                    effects: vec![RuntimePlatformEffect::StoreSaveDefault {
                        payload: self.config_payload(),
                        mode: Some("deferred".into()),
                    }],
                },
            );
        }
        if self.auto_save_flash_pulses_remaining > 0 {
            self.auto_save_flash_pulses_remaining -= 1;
        }
        Ok(messages)
    }

    fn messages_with_effects(
        &mut self,
        effects: Vec<RuntimePlatformEffect>,
    ) -> Result<Vec<RunnerMessage>, String> {
        let mut messages = vec![RunnerMessage::PlatformEffects { effects }];
        messages.extend(self.messages_with_snapshot()?);
        Ok(messages)
    }

    fn messages_with_input_result(
        &mut self,
        result: platform_core::NativeInputResult,
    ) -> Result<Vec<RunnerMessage>, String> {
        let mut messages = Vec::new();
        self.apply_runtime_modulation(&result.mapped_intents, self.active_part_index);
        let events = self.apply_sampler_assignments(
            result.events,
            &result.mapped_intents,
            self.active_part_index,
            result.emitted_events.len(),
        );
        if !events.is_empty() {
            self.event_dot_on = true;
            self.event_dot_pulses_remaining = 6;
            messages.push(RunnerMessage::MusicalEvents { events });
        }
        messages.extend(self.messages_with_snapshot()?);
        Ok(messages)
    }

    fn active_engine_input_result(
        &mut self,
        input: DeviceInput,
    ) -> Result<platform_core::NativeInputResult, String> {
        if self.transport != RuntimeTransportState::Playing && !self.input_events_while_paused {
            let model = self.engine.on_input(input, self.bpm as f32)?;
            return Ok(platform_core::NativeInputResult {
                events: Vec::new(),
                emitted_events: Vec::new(),
                mapped_intents: Vec::new(),
                model,
            });
        }
        let part_index = self.active_part_index;
        let sense = self.sense_parts.get(part_index).cloned();
        let probability_map = self
            .trigger_probability_maps
            .get(part_index)
            .cloned()
            .unwrap_or_default();
        let mut rng = self.trigger_probability_rng;
        let result =
            self.engine
                .on_input_with_events_filtered(input, self.bpm as f32, |intent| {
                    trigger_probability_allows(sense.as_ref(), &probability_map, &mut rng, intent)
                })?;
        self.trigger_probability_rng = rng;
        Ok(result)
    }

    fn active_engine_tick_result(&mut self) -> Result<platform_core::NativeTickResult, String> {
        let part_index = self.active_part_index;
        let sense = self.sense_parts.get(part_index).cloned();
        let probability_map = self
            .trigger_probability_maps
            .get(part_index)
            .cloned()
            .unwrap_or_default();
        let mut rng = self.trigger_probability_rng;
        let result = self.engine.tick_filtered(self.bpm as f32, |intent| {
            trigger_probability_allows(sense.as_ref(), &probability_map, &mut rng, intent)
        })?;
        self.trigger_probability_rng = rng;
        Ok(result)
    }

    fn config_payload(&self) -> Value {
        json!({
            "activeBehavior": self.behavior.id(),
            "runtimeConfig": {
                "activeBehavior": self.behavior.id(),
                "activePartIndex": self.active_part_index,
                "parts": self.part_behavior_ids.iter().enumerate().map(|(index, behavior_id)| {
                    let sense = self.sense_parts.get(index).cloned().unwrap_or_default();
                    let probability_map = self.trigger_probability_maps.get(index).cloned().unwrap_or_default();
                    let auto_name = self.part_auto_names.get(index).copied().unwrap_or(true);
                    let name = self.part_names.get(index).cloned().unwrap_or_else(|| behavior_id.clone());
                    json!({
                        "l1": self.l1_payload_for_part(index, behavior_id),
                        "l2": sense_part_payload(&sense, &probability_map),
                        "paramMods": param_mods_payload(self.param_mods.get(index)),
                        "xy": {
                            "x": param_binding_payload(self.xy_x_binding.as_ref()),
                            "y": param_binding_payload(self.xy_y_binding.as_ref()),
                            "xInvert": self.xy_invert_x,
                            "yInvert": self.xy_invert_y
                        },
                        "autoName": auto_name,
                        "name": name
                    })
                }).collect::<Vec<_>>(),
                "touchFx": {
                    "selected": self.dance_fx_selected.clone(),
                    "assignments": self.dance_fx_assignments.iter().map(|assignment| json!({
                        "x": assignment.x,
                        "y": assignment.y,
                        "config": assignment.config,
                    })).collect::<Vec<_>>()
                },
                "xyTouch": { "x": self.xy_touch.x, "y": self.xy_touch.y, "active": self.xy_touch.active },
                "xyRelease": self.xy_release,
                "instruments": self.instruments.iter().map(|instrument| {
                    let sample_slots = instrument
                        .sample_paths
                        .iter()
                        .map(|path| json!({ "path": path }))
                        .collect::<Vec<_>>();
                    json!({
                        "type": instrument.kind,
                        "noteBehavior": instrument.note_behavior,
                        "autoName": instrument.auto_name,
                        "name": instrument.name,
                        "synth": instrument.synth_config,
                        "sample": {
                            "selectedSlot": instrument.selected_sample_slot,
                            "baseVelocity": instrument.sample_base_velocity,
                            "slots": sample_slots,
                            "assignments": sample_assignments_payload(&instrument.sample_assignments),
                            "tuneSemis": instrument.sample_tune_semis,
                            "amp": { "gainPct": instrument.sample_gain_pct },
                            "ampEnv": instrument.sample_amp_env,
                            "filter": instrument.sample_filter,
                            "filterEnv": instrument.sample_filter_env,
                            "velocityLevelsEnabled": instrument.sample_velocity_levels_enabled,
                            "velocityLevels": {
                                "high": instrument.sample_velocity_high,
                                "medium": instrument.sample_velocity_medium,
                                "low": instrument.sample_velocity_low
                            }
                        },
                        "midi": {
                            "enabled": instrument.midi_enabled,
                            "velocity": instrument.midi_velocity,
                            "durationMs": instrument.midi_duration_ms
                        },
                        "midiEngine": {
                            "velocity": instrument.midi_velocity,
                            "durationMs": instrument.midi_duration_ms
                        },
                        "mixer": {
                            "volume": instrument.volume,
                            "panPos": instrument.pan_pos,
                            "route": instrument.route.clone()
                        }
                    })
                }).collect::<Vec<_>>(),
                "mixer": self.mixer_payload(),
                "masterVolume": self.ui.master_volume,
                "sound": {
                    "noteLengthMs": self.global_sound.note_length_ms,
                    "velocityScalePct": self.global_sound.velocity_scale_pct,
                    "velocityCurve": velocity_curve_id(self.global_sound.velocity_curve),
                    "voiceStealingMode": self.voice_stealing_mode.clone()
                },
                "noteLengthMs": self.global_sound.note_length_ms,
                "velocityScalePct": self.global_sound.velocity_scale_pct,
                "velocityCurve": velocity_curve_id(self.global_sound.velocity_curve),
                "voiceStealingMode": self.voice_stealing_mode.clone(),
                "ghostCells": self.ui.ghost_cells,
                "inputEventsWhilePaused": self.input_events_while_paused,
                "numericDisplayMode": self.ui.numeric_display_mode,
                "screenSleepSeconds": self.ui.screen_sleep_seconds,
                "displayBrightness": self.ui.display_brightness,
                "gridBrightness": self.ui.grid_brightness,
                "buttonBrightness": self.ui.button_brightness,
                "autoSaveDefault": self.auto_save_default,
                "bpm": self.bpm,
                "danceMode": self.dance_mode,
                "auxBindings": aux_bindings_payload(&self.aux_bindings),
                "midi": {
                    "enabled": self.midi_enabled,
                    "outId": self.selected_midi_output_id,
                    "inId": self.selected_midi_input_id,
                    "syncMode": match self.sync_source {
                        SyncSource::Internal => "internal",
                        SyncSource::External => "external",
                    },
                    "clockOutEnabled": self.midi_clock_out_enabled,
                    "clockInEnabled": self.midi_clock_in_enabled,
                    "respondToStartStop": self.midi_respond_to_start_stop
                }
            },
            "mappingConfig": self.mapping_config,
            "system": {
                "danceMode": self.dance_mode
            }
        })
    }

    fn mixer_payload(&self) -> Value {
        json!({
            "buses": self.fx_buses.iter().map(|bus| {
                json!({
                    "name": bus.name,
                    "slot1": fx_slot_payload(&bus.slot1_type),
                    "slot2": fx_slot_payload(&bus.slot2_type),
                    "panPos": bus.pan_pos,
                    "autoName": bus.auto_name
                })
            }).collect::<Vec<_>>(),
            "master": {
                "slots": self.global_fx_slots.iter().map(|slot_type| {
                    fx_slot_payload(slot_type)
                }).collect::<Vec<_>>()
            }
        })
    }

    fn apply_factory_payload(&mut self) -> Result<(), String> {
        self.apply_config_payload(native_factory_payload())?;
        self.toast = Some(NativeToast {
            message: "Factory loaded".into(),
            offset: 0,
        });
        Ok(())
    }

    fn platform_effect_for_action(&self, action: &str) -> Option<RuntimePlatformEffect> {
        match action {
            "preset.refresh" => Some(RuntimePlatformEffect::StoreListPresets),
            "default.load" => Some(RuntimePlatformEffect::StoreLoadDefault),
            "default.save" => Some(RuntimePlatformEffect::StoreSaveDefault {
                payload: self.config_payload(),
                mode: None,
            }),
            "preset.saveCurrent" => self.current_preset_name.as_ref().map(|name| {
                RuntimePlatformEffect::StoreSavePreset {
                    name: name.clone(),
                    payload: self.config_payload(),
                    mode: Some("overwrite".into()),
                }
            }),
            action if action.starts_with("preset.load:") => action
                .strip_prefix("preset.load:")
                .map(|name| RuntimePlatformEffect::StoreLoadPreset { name: name.into() }),
            action if action.starts_with("preset.delete:") => action
                .strip_prefix("preset.delete:")
                .map(|name| RuntimePlatformEffect::StoreDeletePreset { name: name.into() }),
            "midi.panic" => Some(RuntimePlatformEffect::MidiPanic),
            action if action.starts_with("midi.output:") => {
                let id = action.strip_prefix("midi.output:").unwrap_or_default();
                Some(RuntimePlatformEffect::MidiSelectOutput {
                    id: if id.is_empty() { None } else { Some(id.into()) },
                })
            }
            action if action.starts_with("midi.input:") => {
                let id = action.strip_prefix("midi.input:").unwrap_or_default();
                Some(RuntimePlatformEffect::MidiSelectInput {
                    id: if id.is_empty() { None } else { Some(id.into()) },
                })
            }
            _ => None,
        }
    }

    fn handle_sample_action(
        &mut self,
        action: &str,
    ) -> Result<Option<RuntimePlatformEffect>, String> {
        if action == "factory.load" {
            self.apply_factory_payload()?;
            return Ok(None);
        }
        if action == "dance.fx.map" {
            let config = self.dance_fx_selected.clone();
            self.dance_fx_assign = Some(config.clone());
            self.active_dance_mode = "fx".into();
            self.toast = Some(NativeToast {
                message: format!("Map FX: {}", dance_fx_type(&config)),
                offset: 0,
            });
            return Ok(None);
        }
        if let Some(rest) = action.strip_prefix("sample.open:") {
            let (instrument_slot, sample_slot, dir) = parse_sample_action(rest)?;
            let dir = dir.unwrap_or_default();
            self.sample_browser = Some(NativeSampleBrowser {
                instrument_slot,
                sample_slot,
                dir: dir.clone(),
                entries: vec![],
            });
            self.menu.rebuild(self.menu_config());
            return Ok(Some(RuntimePlatformEffect::SampleListRequest {
                instrument_slot,
                sample_slot,
                dir,
            }));
        }
        if let Some(rest) = action.strip_prefix("sample.enter:") {
            let (instrument_slot, sample_slot, dir) = parse_sample_action(rest)?;
            let dir = dir.unwrap_or_default();
            self.sample_browser = Some(NativeSampleBrowser {
                instrument_slot,
                sample_slot,
                dir: dir.clone(),
                entries: vec![],
            });
            self.menu.rebuild(self.menu_config());
            return Ok(Some(RuntimePlatformEffect::SampleListRequest {
                instrument_slot,
                sample_slot,
                dir,
            }));
        }
        if let Some(rest) = action.strip_prefix("sample.up:") {
            let (instrument_slot, sample_slot, _) = parse_sample_action(rest)?;
            let dir = self
                .sample_browser
                .as_ref()
                .map(|browser| parent_dir(&browser.dir))
                .unwrap_or_default();
            self.sample_browser = Some(NativeSampleBrowser {
                instrument_slot,
                sample_slot,
                dir: dir.clone(),
                entries: vec![],
            });
            self.menu.rebuild(self.menu_config());
            return Ok(Some(RuntimePlatformEffect::SampleListRequest {
                instrument_slot,
                sample_slot,
                dir,
            }));
        }
        if let Some(rest) = action.strip_prefix("sample.pick:") {
            let (instrument_slot, sample_slot, path) = parse_sample_action(rest)?;
            let Some(path) = path else {
                return Ok(None);
            };
            if let Some(instrument) = self.instruments.get_mut(instrument_slot) {
                if sample_slot < instrument.sample_paths.len() {
                    instrument.sample_paths[sample_slot] = Some(path);
                    self.menu.rebuild(self.menu_config());
                }
            }
            return Ok(None);
        }
        if let Some(rest) = action.strip_prefix("sample.assign:") {
            let (instrument_slot, sample_slot, _) = parse_sample_action(rest)?;
            self.sample_assign = Some((instrument_slot, sample_slot));
            return Ok(None);
        }
        if let Some(rest) = action.strip_prefix("trigger.probability.assign:") {
            if let Ok(part_index) = rest.parse::<usize>() {
                self.trigger_probability_assign = Some(part_index.min(GRID_HEIGHT - 1));
            }
            return Ok(None);
        }
        if let Some(rest) = action.strip_prefix("synth.preset:") {
            let mut parts = rest.splitn(2, ':');
            let slot = parts.next().and_then(|value| value.parse::<usize>().ok());
            let preset = parts.next();
            if let (Some(slot), Some(preset)) = (slot, preset) {
                self.load_synth_preset(slot, preset);
            }
            return Ok(None);
        }
        if let Some(rest) = action.strip_prefix("sample.preview:") {
            let (instrument_slot, sample_slot, path) = parse_sample_action(rest)?;
            if let Some(path) = path {
                return Ok(Some(RuntimePlatformEffect::AudioCommand {
                    command: RuntimeAudioCommand::SamplePreview {
                        instrument_slot,
                        sample_slot,
                        path,
                        velocity: 100,
                    },
                }));
            }
            return Ok(None);
        }
        Ok(None)
    }

    fn load_synth_preset(&mut self, slot: usize, preset: &str) {
        let Some(instrument) = self.instruments.get_mut(slot) else {
            return;
        };
        let synth_config = synth_preset_config(preset);
        let gain = synth_config
            .get("amp")
            .and_then(|amp| amp.get("gainPct"))
            .and_then(Value::as_u64)
            .unwrap_or(80) as u8;
        instrument.kind = "synth".into();
        if instrument.auto_name {
            instrument.name = "synth".into();
        }
        instrument.synth_config = synth_config;
        instrument.synth_gain_pct = gain;
        self.menu.rebuild(self.menu_config());
    }

    fn sample_open_effect_for_current_group(&mut self) -> Option<RuntimePlatformEffect> {
        let key = self
            .menu
            .current_key()?
            .strip_prefix("sample.choose:")?
            .to_string();
        self.sample_open_effect_for_key(&key)
    }

    fn sample_open_effect_for_key(&mut self, key: &str) -> Option<RuntimePlatformEffect> {
        let key = key.strip_prefix("sample.choose:").unwrap_or(key);
        let (instrument_slot, sample_slot, _) = parse_sample_action(key).ok()?;
        let dir = self
            .sample_browser
            .as_ref()
            .filter(|browser| {
                browser.instrument_slot == instrument_slot && browser.sample_slot == sample_slot
            })
            .map(|browser| browser.dir.clone())
            .unwrap_or_default();
        self.sample_browser = Some(NativeSampleBrowser {
            instrument_slot,
            sample_slot,
            dir: dir.clone(),
            entries: vec![],
        });
        self.menu.rebuild(self.menu_config());
        Some(RuntimePlatformEffect::SampleListRequest {
            instrument_slot,
            sample_slot,
            dir,
        })
    }

    fn preview_selected_sample(&self) -> Result<Option<RuntimePlatformEffect>, String> {
        let Some(NativeMenuAction::PlatformEffect(action)) = self.menu.snapshot().selected_action
        else {
            return Ok(None);
        };
        let Some(rest) = action.strip_prefix("sample.pick:") else {
            return Ok(None);
        };
        let (instrument_slot, sample_slot, path) = parse_sample_action(rest)?;
        let Some(path) = path else {
            return Ok(None);
        };
        Ok(Some(RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::SamplePreview {
                instrument_slot,
                sample_slot,
                path,
                velocity: 100,
            },
        }))
    }

    fn execute_menu_action(
        &mut self,
        action: NativeMenuAction,
    ) -> Result<Option<RuntimePlatformEffect>, String> {
        match action {
            NativeMenuAction::Noop => Ok(None),
            NativeMenuAction::BehaviorAction(action_type) => {
                self.trigger_behavior_action(action_type)?;
                Ok(None)
            }
            NativeMenuAction::PlatformEffect(action_type) => {
                if let Some(effect) = self.handle_sample_action(&action_type)? {
                    Ok(Some(effect))
                } else {
                    Ok(self.platform_effect_for_action(&action_type))
                }
            }
            NativeMenuAction::SetParamBinding { target, binding } => {
                self.set_param_binding_target(&target, Some(native_binding_from_spec(binding)));
                Ok(None)
            }
            NativeMenuAction::ClearParamBinding { target } => {
                self.set_param_binding_target(&target, None);
                Ok(None)
            }
            NativeMenuAction::SetAuxClick { index, action } => {
                self.set_aux_click_target(index, action.map(|action| *action));
                Ok(None)
            }
            NativeMenuAction::ResetBehavior => {
                self.seed_visible_state()?;
                Ok(None)
            }
        }
    }

    fn set_aux_click_target(&mut self, index: usize, action: Option<NativeMenuAction>) {
        if index >= self.aux_bindings.len() {
            return;
        }
        let turn_key = self
            .aux_bindings
            .get(index)
            .and_then(|binding| binding.as_ref())
            .and_then(|binding| binding.turn_key.clone());
        self.aux_bindings[index] = if turn_key.is_some() || action.is_some() {
            Some(NativeAuxBinding {
                turn_key,
                press_action: action.clone(),
            })
        } else {
            None
        };
        self.toast = Some(NativeToast {
            message: format!("Aux {} click mapped", index + 1),
            offset: 0,
        });
        self.config_dirty = true;
        self.menu.rebuild(self.menu_config());
    }

    fn set_param_binding_target(&mut self, target: &str, binding: Option<NativeParamBinding>) {
        if let Some(rest) = target.strip_prefix("param:") {
            let parts = rest.split(':').collect::<Vec<_>>();
            if parts.len() == 3 {
                let part = parts[0]
                    .parse::<usize>()
                    .unwrap_or(self.active_part_index)
                    .min(GRID_HEIGHT - 1);
                let slot = parts[2].parse::<usize>().unwrap_or(0).min(1);
                if let Some(param_mods) = self.param_mods.get_mut(part) {
                    match parts[1] {
                        "x" => param_mods.x[slot] = binding.clone(),
                        "y" => param_mods.y[slot] = binding.clone(),
                        _ => {}
                    }
                }
            }
        } else if target == "xy:x" {
            self.xy_x_binding = binding.clone();
        } else if target == "xy:y" {
            self.xy_y_binding = binding.clone();
        } else if let Some(rest) = target.strip_prefix("aux:") {
            let parts = rest.split(':').collect::<Vec<_>>();
            if parts.len() == 2 && parts[1] == "turn" {
                let index = parts[0].parse::<usize>().unwrap_or(0).min(3);
                let press_action = self
                    .aux_bindings
                    .get(index)
                    .and_then(|binding| binding.as_ref())
                    .and_then(|binding| binding.press_action.clone());
                self.aux_bindings[index] = if let Some(binding) = binding.clone() {
                    Some(NativeAuxBinding {
                        turn_key: Some(binding.key),
                        press_action,
                    })
                } else if press_action.is_some() {
                    Some(NativeAuxBinding {
                        turn_key: None,
                        press_action,
                    })
                } else {
                    None
                };
            }
        }
        let label = binding
            .as_ref()
            .and_then(|binding| binding.label.as_deref())
            .unwrap_or("none");
        self.toast = Some(NativeToast {
            message: format!("Mapped {label}"),
            offset: 0,
        });
        self.config_dirty = true;
        self.menu.rebuild(self.menu_config());
    }

    fn aux_index(id: Option<&str>) -> Option<usize> {
        match id {
            Some("aux1") => Some(0),
            Some("aux2") => Some(1),
            Some("aux3") => Some(2),
            Some("aux4") => Some(3),
            _ => None,
        }
    }

    fn bind_aux_from_current(&mut self, index: usize) -> bool {
        let (turn_key, press_action) = self.menu.current_binding_target();
        if turn_key.is_none() && press_action.is_none() {
            return false;
        }
        if let Some(slot) = self.aux_bindings.get_mut(index) {
            *slot = Some(NativeAuxBinding {
                turn_key,
                press_action,
            });
            return true;
        }
        false
    }

    fn handle_param_mod_grid_press(&mut self, x: usize, y: usize) -> bool {
        let Some(mut binding) = self
            .menu
            .current_param_binding()
            .map(native_binding_from_spec)
        else {
            return false;
        };
        if let Some(field) = binding.key.strip_prefix("behavior.") {
            binding.key = format!("parts.{}.l1.behaviorConfig.{field}", self.active_part_index);
        }
        self.apply_param_mod_mapping(x, y, binding)
    }

    fn apply_param_mod_mapping(&mut self, x: usize, y: usize, binding: NativeParamBinding) -> bool {
        let targets = param_mod_grid_targets(x, y);
        if targets.is_empty() {
            return false;
        }
        let Some(param_mods) = self.param_mods.get_mut(self.active_part_index) else {
            return false;
        };
        let current = match targets[0].0 {
            "x" => param_mods.x[targets[0].1].as_ref(),
            "y" => param_mods.y[targets[0].1].as_ref(),
            _ => None,
        };
        let mode = param_mod_next_toggle_mode(current, &binding.key);
        for (axis, slot) in &targets {
            let next = if mode == "clear" {
                None
            } else {
                let mut next = binding.clone();
                next.invert = mode == "invert";
                Some(next)
            };
            match *axis {
                "x" => param_mods.x[*slot] = next,
                "y" => param_mods.y[*slot] = next,
                _ => {}
            }
        }
        let axis_label = if targets.len() == 2 {
            format!("X/Y Slot {}", targets[0].1 + 1)
        } else {
            format!("{} Slot {}", targets[0].0.to_uppercase(), targets[0].1 + 1)
        };
        let action = match mode {
            "clear" => "cleared",
            "invert" => "inverted",
            _ => "mapped",
        };
        let label = binding.label.as_deref().unwrap_or(&binding.key);
        self.toast = Some(NativeToast {
            message: format!("{axis_label}: {label} {action}"),
            offset: 0,
        });
        self.config_dirty = true;
        self.menu.rebuild(self.menu_config());
        true
    }

    fn handle_aux_turn(&mut self, index: usize, delta: i8) -> Result<(), String> {
        if delta == 0 {
            return Ok(());
        }
        let Some(Some(binding)) = self.aux_bindings.get(index) else {
            self.toast = Some(NativeToast {
                message: format!("Aux {} turn not bound", index + 1),
                offset: 0,
            });
            return Ok(());
        };
        let Some(key) = binding.turn_key.clone() else {
            self.toast = Some(NativeToast {
                message: format!("Aux {} turn not bound", index + 1),
                offset: 0,
            });
            return Ok(());
        };
        if self.menu.turn_key(&key, delta) {
            self.apply_menu_state()?;
            let value = self
                .menu
                .value_for_key(&key)
                .or_else(|| {
                    self.menu
                        .number_for_key(&key)
                        .map(|value| value.to_string())
                })
                .unwrap_or_else(|| "changed".into());
            self.toast = Some(NativeToast {
                message: format!("Aux {} {value}", index + 1),
                offset: 0,
            });
        }
        Ok(())
    }

    fn handle_aux_press(&mut self, index: usize) -> Result<Option<RuntimePlatformEffect>, String> {
        if self.ui.fn_held {
            self.bind_aux_from_current(index);
            return Ok(None);
        }
        let Some(Some(binding)) = self.aux_bindings.get(index) else {
            self.toast = Some(NativeToast {
                message: format!("Aux {} click not bound", index + 1),
                offset: 0,
            });
            return Ok(None);
        };
        let Some(action) = binding.press_action.clone() else {
            self.toast = Some(NativeToast {
                message: format!("Aux {} click not bound", index + 1),
                offset: 0,
            });
            return Ok(None);
        };
        self.execute_menu_action(action)
    }

    fn apply_store_result(&mut self, result: RuntimeStoreResult) -> Result<(), String> {
        match result {
            RuntimeStoreResult::LoadDefaultResult {
                payload: Some(payload),
            } => {
                self.apply_config_payload(payload)?;
            }
            RuntimeStoreResult::LoadPresetResult { name, payload } => {
                if let Some(payload) = payload {
                    self.apply_config_payload(payload)?;
                }
                self.current_preset_name = Some(name);
            }
            RuntimeStoreResult::SavePresetResult { name, .. } => {
                self.current_preset_name = Some(name);
            }
            RuntimeStoreResult::SaveDefaultResult { ok, is_auto: _ } if ok => {
                self.auto_save_flash_serial = self.auto_save_flash_serial.wrapping_add(1);
                self.auto_save_flash_pulses_remaining = 8;
                self.toast = Some(NativeToast {
                    message: "Saved default".into(),
                    offset: 0,
                });
            }
            RuntimeStoreResult::StoreError { message } => {
                self.toast = Some(NativeToast { message, offset: 0 });
            }
            RuntimeStoreResult::ListPresetsResult { names } => {
                self.preset_names = names;
                self.menu.rebuild(self.menu_config());
            }
            RuntimeStoreResult::MidiListOutputsResult { outputs } => {
                self.midi_outputs = outputs;
                self.menu.rebuild(self.menu_config());
            }
            RuntimeStoreResult::MidiListInputsResult { inputs } => {
                self.midi_inputs = inputs;
                self.menu.rebuild(self.menu_config());
            }
            RuntimeStoreResult::MidiStatus {
                ok,
                message,
                selected_out_id,
                selected_in_id,
            } => {
                self.midi_status = Some(if ok {
                    "MIDI ok".into()
                } else {
                    message.unwrap_or_else(|| "MIDI error".into())
                });
                self.selected_midi_output_id = selected_out_id;
                self.selected_midi_input_id = selected_in_id;
            }
            RuntimeStoreResult::SampleListResult {
                instrument_slot,
                sample_slot,
                dir,
                entries,
            } => {
                self.sample_browser = Some(NativeSampleBrowser {
                    instrument_slot,
                    sample_slot,
                    dir,
                    entries,
                });
                self.menu.rebuild(self.menu_config());
            }
            RuntimeStoreResult::SampleListError {
                instrument_slot,
                sample_slot,
                dir,
                ..
            } => {
                self.sample_browser = Some(NativeSampleBrowser {
                    instrument_slot,
                    sample_slot,
                    dir,
                    entries: vec![],
                });
                self.menu.rebuild(self.menu_config());
            }
            _ => {}
        }
        Ok(())
    }

    fn apply_config_payload(&mut self, payload: Value) -> Result<(), String> {
        let runtime = payload.get("runtimeConfig").unwrap_or(&payload);
        let desired_active_part_index = runtime
            .get("activePartIndex")
            .and_then(Value::as_u64)
            .map(|value| (value as usize).min(GRID_HEIGHT.saturating_sub(1)))
            .unwrap_or(self.active_part_index);
        if let Some(active_part_index) = runtime.get("activePartIndex").and_then(Value::as_u64) {
            self.active_part_index =
                (active_part_index as usize).min(GRID_HEIGHT.saturating_sub(1));
        }
        if let Some(parts) = runtime.get("parts").and_then(Value::as_array) {
            for (index, part) in parts.iter().take(GRID_HEIGHT).enumerate() {
                let l1 = part.get("l1");
                if let Some(behavior_id) = part
                    .get("l1")
                    .and_then(|l1| l1.get("behaviorId"))
                    .and_then(Value::as_str)
                {
                    if platform_core::get_native_behavior(behavior_id).is_some() {
                        self.part_behavior_ids[index] = behavior_id.into();
                    }
                }
                if let Some(auto_name) = part.get("autoName").and_then(Value::as_bool) {
                    if let Some(target) = self.part_auto_names.get_mut(index) {
                        *target = auto_name;
                    }
                }
                if let Some(name) = part.get("name").and_then(Value::as_str) {
                    if let Some(target) = self.part_names.get_mut(index) {
                        *target = name.into();
                    }
                } else if self.part_auto_names.get(index).copied().unwrap_or(true) {
                    if let Some(target) = self.part_names.get_mut(index) {
                        *target = self
                            .part_behavior_ids
                            .get(index)
                            .cloned()
                            .unwrap_or_else(|| self.behavior.id().into());
                    }
                }
                if let Some(l2) = part.get("l2") {
                    if let Some(sense_part) = self.sense_parts.get_mut(index) {
                        apply_sense_payload(sense_part, l2);
                    }
                    if let Some(target) = self.trigger_probability_maps.get_mut(index) {
                        if let Some(map) = l2.get("triggerProbabilityMap").and_then(Value::as_array)
                        {
                            apply_trigger_probability_map_payload(target, map);
                        } else if let Some(gates) = l1
                            .and_then(|l1| l1.get("triggerGates"))
                            .and_then(Value::as_array)
                        {
                            apply_legacy_trigger_gates_payload(target, gates);
                        }
                    }
                }
                if let (Some(l1), Some(behavior_id)) = (l1, self.part_behavior_ids.get(index)) {
                    if let Some(save_grid_state) = l1.get("saveGridState").and_then(Value::as_bool)
                    {
                        if let Some(target) = self.save_grid_states.get_mut(index) {
                            *target = save_grid_state;
                        }
                    }
                    if let Some(step_rate) = l1.get("stepRate").and_then(Value::as_str) {
                        if let Some(part_step) = self.part_algorithm_step_pulses.get_mut(index) {
                            *part_step = note_unit_to_pulses(step_rate);
                        }
                    }
                    if let Some(config) = l1.get("behaviorConfig") {
                        if let Some(target) = self.part_behavior_configs.get_mut(index) {
                            *target = config.clone();
                        }
                    }
                    let engine = self.rebuild_part_engine_from_payload(index, behavior_id, l1)?;
                    if let Some(slot) = self.part_engines.get_mut(index) {
                        *slot = Some(engine);
                    }
                }
                if let Some(param_mods) = part.get("paramMods") {
                    if let Some(target) = self.param_mods.get_mut(index) {
                        *target = param_mods_from_payload(param_mods);
                    }
                }
            }
        }
        if let Some(touch_fx) = runtime.get("touchFx") {
            self.apply_touch_fx_payload(touch_fx);
        }
        if let Some(xy_touch) = runtime.get("xyTouch") {
            if let Some(x) = xy_touch.get("x").and_then(Value::as_f64) {
                self.xy_touch.x = (x as f32).clamp(0.0, 1.0);
            }
            if let Some(y) = xy_touch.get("y").and_then(Value::as_f64) {
                self.xy_touch.y = (y as f32).clamp(0.0, 1.0);
            }
            if let Some(active) = xy_touch.get("active").and_then(Value::as_bool) {
                self.xy_touch.active = active;
            }
        }
        if let Some(xy_release) = runtime.get("xyRelease").and_then(Value::as_str) {
            if matches!(xy_release, "sample-hold" | "reset-center") {
                self.xy_release = xy_release.into();
            }
        }
        if let Some(active_part) = runtime
            .get("parts")
            .and_then(Value::as_array)
            .and_then(|parts| parts.get(desired_active_part_index))
            .and_then(|part| part.get("xy"))
        {
            self.xy_x_binding = active_part.get("x").and_then(param_binding_from_payload);
            self.xy_y_binding = active_part.get("y").and_then(param_binding_from_payload);
            if let Some(invert) = active_part.get("xInvert").and_then(Value::as_bool) {
                self.xy_invert_x = invert;
            }
            if let Some(invert) = active_part.get("yInvert").and_then(Value::as_bool) {
                self.xy_invert_y = invert;
            }
        }
        self.active_part_index = desired_active_part_index;
        self.algorithm_step_pulses = self
            .part_algorithm_step_pulses
            .get(self.active_part_index)
            .copied()
            .unwrap_or(DEFAULT_ALGORITHM_STEP_PULSES);
        if let Some(Some(engine)) = self.part_engines.get_mut(desired_active_part_index) {
            let placeholder = Self::build_engine(
                self.behavior,
                self.behavior_config.clone(),
                self.interpretation_profile.clone(),
                self.mapping_config.clone(),
                self.global_sound.clone(),
                self.note_behaviors.clone(),
                desired_active_part_index,
            )?;
            self.engine = std::mem::replace(engine, placeholder);
        }
        if let Some(behavior_id) = self.part_behavior_ids.get(desired_active_part_index) {
            if let Some(behavior) = platform_core::get_native_behavior(behavior_id) {
                self.behavior = behavior;
            }
        }
        self.behavior_config = self
            .part_behavior_configs
            .get(desired_active_part_index)
            .filter(|config| !config.is_null())
            .cloned()
            .unwrap_or_else(|| self.behavior_config.clone());
        let incoming_pan_positions = runtime.get("panPositions").and_then(Value::as_u64);
        if let Some(instruments) = runtime.get("instruments").and_then(Value::as_array) {
            for (index, slot) in instruments.iter().take(self.instruments.len()).enumerate() {
                if let Some(instrument) = self.instruments.get_mut(index) {
                    if let Some(kind) = slot.get("type").and_then(Value::as_str) {
                        if matches!(kind, "none" | "synth" | "sampler" | "midi") {
                            instrument.kind = kind.into();
                        }
                    }
                    if let Some(note_behavior) = slot.get("noteBehavior").and_then(Value::as_str) {
                        if matches!(note_behavior, "oneshot" | "hold") {
                            instrument.note_behavior = note_behavior.into();
                        }
                    }
                    if let Some(auto_name) = slot.get("autoName").and_then(Value::as_bool) {
                        instrument.auto_name = auto_name;
                    }
                    if let Some(name) = slot.get("name").and_then(Value::as_str) {
                        instrument.name = name.into();
                    } else if instrument.auto_name {
                        instrument.name = derive_instrument_name(index, &instrument.kind);
                    }
                    if let Some(mixer) = slot.get("mixer") {
                        if let Some(volume) = mixer.get("volume").and_then(Value::as_u64) {
                            instrument.volume = (volume as u8).min(127);
                        }
                        if let Some(pan_pos) = mixer.get("panPos").and_then(Value::as_u64) {
                            instrument.pan_pos =
                                sanitize_pan_position_payload(pan_pos, incoming_pan_positions);
                        }
                        if let Some(route) = mixer.get("route").and_then(Value::as_str) {
                            instrument.route = normalize_route(route);
                        }
                    }
                    if let Some(sample) = slot.get("sample") {
                        if let Some(selected_slot) =
                            sample.get("selectedSlot").and_then(Value::as_u64)
                        {
                            instrument.selected_sample_slot = (selected_slot as usize).min(7);
                        }
                        if let Some(base_velocity) =
                            sample.get("baseVelocity").and_then(Value::as_u64)
                        {
                            instrument.sample_base_velocity = (base_velocity as u8).clamp(1, 127);
                        }
                        if let Some(slots) = sample.get("slots").and_then(Value::as_array) {
                            for (sample_index, sample_slot) in slots.iter().take(8).enumerate() {
                                instrument.sample_paths[sample_index] = sample_slot
                                    .get("path")
                                    .and_then(Value::as_str)
                                    .map(str::to_string);
                            }
                        }
                        if let Some(assignments) =
                            sample.get("assignments").and_then(Value::as_array)
                        {
                            instrument.sample_assignments = assignments
                                .iter()
                                .filter_map(sample_assignment_from_payload)
                                .collect();
                        }
                        if let Some(tune) = sample.get("tuneSemis").and_then(Value::as_i64) {
                            instrument.sample_tune_semis = (tune as i8).clamp(-24, 24);
                        }
                        if let Some(gain) = sample
                            .get("amp")
                            .and_then(|amp| amp.get("gainPct"))
                            .and_then(Value::as_u64)
                        {
                            instrument.sample_gain_pct = (gain as u8).min(100);
                        }
                        if let Some(amp_env) =
                            sample.get("ampEnv").filter(|value| value.is_object())
                        {
                            instrument.sample_amp_env = amp_env.clone();
                        }
                        if let Some(filter) = sample.get("filter").filter(|value| value.is_object())
                        {
                            instrument.sample_filter = filter.clone();
                        }
                        if let Some(filter_env) =
                            sample.get("filterEnv").filter(|value| value.is_object())
                        {
                            instrument.sample_filter_env = filter_env.clone();
                        }
                        if let Some(enabled) =
                            sample.get("velocityLevelsEnabled").and_then(Value::as_bool)
                        {
                            instrument.sample_velocity_levels_enabled = enabled;
                        }
                        if let Some(levels) = sample.get("velocityLevels") {
                            if let Some(high) = levels.get("high").and_then(Value::as_u64) {
                                instrument.sample_velocity_high = (high as u8).clamp(1, 127);
                            }
                            if let Some(medium) = levels.get("medium").and_then(Value::as_u64) {
                                instrument.sample_velocity_medium = (medium as u8).clamp(1, 127);
                            }
                            if let Some(low) = levels.get("low").and_then(Value::as_u64) {
                                instrument.sample_velocity_low = (low as u8).clamp(1, 127);
                            }
                        }
                    }
                    if let Some(synth) = slot.get("synth") {
                        instrument.synth_config = synth.clone();
                        if let Some(gain) = synth
                            .get("amp")
                            .and_then(|amp| amp.get("gainPct"))
                            .and_then(Value::as_u64)
                        {
                            instrument.synth_gain_pct = (gain as u8).min(100);
                        }
                    }
                    if let Some(midi) = slot.get("midi") {
                        if let Some(enabled) = midi.get("enabled").and_then(Value::as_bool) {
                            instrument.midi_enabled = enabled;
                        }
                        if let Some(velocity) = midi.get("velocity").and_then(Value::as_u64) {
                            instrument.midi_velocity = (velocity as u8).clamp(1, 127);
                        }
                        if let Some(duration_ms) = midi.get("durationMs").and_then(Value::as_u64) {
                            instrument.midi_duration_ms = (duration_ms as u16).clamp(10, 5000);
                        }
                    }
                    if let Some(midi_engine) = slot.get("midiEngine") {
                        if let Some(velocity) = midi_engine.get("velocity").and_then(Value::as_u64)
                        {
                            instrument.midi_velocity = (velocity as u8).clamp(1, 127);
                        }
                        if let Some(duration_ms) =
                            midi_engine.get("durationMs").and_then(Value::as_u64)
                        {
                            instrument.midi_duration_ms = (duration_ms as u16).clamp(10, 5000);
                        }
                    }
                }
            }
        }
        if let Some(mixer) = runtime.get("mixer") {
            if let Some(buses) = mixer.get("buses").and_then(Value::as_array) {
                for (index, payload) in buses.iter().take(self.fx_buses.len()).enumerate() {
                    if let Some(bus) = self.fx_buses.get_mut(index) {
                        if let Some(slot1) = payload
                            .get("slot1")
                            .and_then(|slot| slot.get("type"))
                            .and_then(Value::as_str)
                        {
                            bus.slot1_type = slot1.into();
                        }
                        if let Some(slot2) = payload
                            .get("slot2")
                            .and_then(|slot| slot.get("type"))
                            .and_then(Value::as_str)
                        {
                            bus.slot2_type = slot2.into();
                        }
                        if let Some(pan_pos) = payload.get("panPos").and_then(Value::as_u64) {
                            bus.pan_pos = (pan_pos as u8).min(32);
                        }
                        if let Some(auto_name) = payload.get("autoName").and_then(Value::as_bool) {
                            bus.auto_name = auto_name;
                        }
                        if let Some(name) = payload.get("name").and_then(Value::as_str) {
                            bus.name = name.into();
                        } else if bus.auto_name {
                            bus.name = derive_bus_name(bus);
                        }
                    }
                }
            }
            if let Some(slots) = mixer
                .get("master")
                .and_then(|master| master.get("slots"))
                .and_then(Value::as_array)
            {
                for (index, payload) in slots.iter().take(self.global_fx_slots.len()).enumerate() {
                    if let Some(slot_type) = payload.get("type").and_then(Value::as_str) {
                        self.global_fx_slots[index] = slot_type.into();
                    }
                }
            }
        }
        if let Some(master_volume) = runtime.get("masterVolume").and_then(Value::as_u64) {
            self.ui.master_volume = (master_volume as u8).min(100);
        }
        let sound = runtime.get("sound");
        if let Some(note_length_ms) = sound
            .and_then(|sound| sound.get("noteLengthMs"))
            .or_else(|| runtime.get("noteLengthMs"))
            .and_then(Value::as_u64)
        {
            self.global_sound.note_length_ms = (note_length_ms as u32).clamp(30, 2000);
        }
        if let Some(velocity_scale_pct) = sound
            .and_then(|sound| sound.get("velocityScalePct"))
            .or_else(|| runtime.get("velocityScalePct"))
            .and_then(Value::as_u64)
        {
            self.global_sound.velocity_scale_pct = (velocity_scale_pct as u16).min(200);
        }
        if let Some(velocity_curve) = sound
            .and_then(|sound| sound.get("velocityCurve"))
            .or_else(|| runtime.get("velocityCurve"))
            .and_then(Value::as_str)
        {
            self.global_sound.velocity_curve = velocity_curve_from_id(velocity_curve);
        }
        if let Some(voice_stealing_mode) = sound
            .and_then(|sound| sound.get("voiceStealingMode"))
            .or_else(|| runtime.get("voiceStealingMode"))
            .and_then(Value::as_str)
        {
            if matches!(
                voice_stealing_mode,
                "off" | "lenient" | "balanced" | "aggressive"
            ) {
                self.voice_stealing_mode = voice_stealing_mode.into();
            }
        }
        if let Some(display_brightness) = runtime.get("displayBrightness").and_then(Value::as_u64) {
            self.ui.display_brightness = (display_brightness as u8).min(100);
        }
        if let Some(grid_brightness) = runtime.get("gridBrightness").and_then(Value::as_u64) {
            self.ui.grid_brightness = (grid_brightness as u8).min(100);
        }
        if let Some(button_brightness) = runtime.get("buttonBrightness").and_then(Value::as_u64) {
            self.ui.button_brightness = (button_brightness as u8).min(100);
        }
        if let Some(input_events_while_paused) = runtime
            .get("inputEventsWhilePaused")
            .and_then(Value::as_bool)
        {
            self.input_events_while_paused = input_events_while_paused;
        }
        if let Some(numeric_display_mode) =
            runtime.get("numericDisplayMode").and_then(Value::as_str)
        {
            if matches!(numeric_display_mode, "bar" | "numbers" | "bar+numbers") {
                self.ui.numeric_display_mode = numeric_display_mode.into();
            }
        }
        if let Some(screen_sleep_seconds) =
            runtime.get("screenSleepSeconds").and_then(Value::as_u64)
        {
            self.ui.screen_sleep_seconds = (screen_sleep_seconds as u16).min(600);
        }
        if let Some(aux_bindings) = runtime.get("auxBindings") {
            apply_aux_bindings_payload(&mut self.aux_bindings, aux_bindings);
        }
        if let Some(bpm) = runtime.get("bpm").and_then(Value::as_f64) {
            self.bpm = bpm.clamp(20.0, 300.0);
        }
        if let Some(dance_mode) = runtime.get("danceMode").and_then(Value::as_str) {
            if matches!(
                dance_mode,
                "none" | "mix" | "pan" | "fx" | "trigger-gate" | "xy"
            ) {
                self.dance_mode = dance_mode.into();
                self.active_dance_mode = "none".into();
            }
        }
        if let Some(midi) = runtime.get("midi") {
            if let Some(enabled) = midi.get("enabled").and_then(Value::as_bool) {
                self.midi_enabled = enabled;
            }
            if let Some(out_id) = midi.get("outId") {
                self.selected_midi_output_id = out_id.as_str().map(str::to_string);
            }
            if let Some(in_id) = midi.get("inId") {
                self.selected_midi_input_id = in_id.as_str().map(str::to_string);
            }
            if let Some(sync_mode) = midi.get("syncMode").and_then(Value::as_str) {
                self.sync_source = if sync_mode == "external" {
                    SyncSource::External
                } else {
                    SyncSource::Internal
                };
            }
            if let Some(clock_out_enabled) = midi.get("clockOutEnabled").and_then(Value::as_bool) {
                self.midi_clock_out_enabled = clock_out_enabled;
            }
            if let Some(clock_in_enabled) = midi.get("clockInEnabled").and_then(Value::as_bool) {
                self.midi_clock_in_enabled = clock_in_enabled;
            }
            if let Some(respond_to_start_stop) =
                midi.get("respondToStartStop").and_then(Value::as_bool)
            {
                self.midi_respond_to_start_stop = respond_to_start_stop;
            }
        }
        if let Some(mapping_config) = payload.get("mappingConfig") {
            self.mapping_config = serde_json::from_value(mapping_config.clone())
                .unwrap_or_else(|_| default_mapping_config());
        }
        let active_behavior_id = self
            .part_behavior_ids
            .get(self.active_part_index)
            .cloned()
            .or_else(|| {
                payload
                    .get("activeBehavior")
                    .and_then(Value::as_str)
                    .map(String::from)
            })
            .unwrap_or_else(|| self.behavior.id().into());
        let behavior = platform_core::get_native_behavior(&active_behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{active_behavior_id}`"))?;
        self.behavior = behavior;
        if let Some(active_l1) = runtime
            .get("parts")
            .and_then(Value::as_array)
            .and_then(|parts| parts.get(self.active_part_index))
            .and_then(|part| part.get("l1"))
        {
            self.behavior_config = active_l1
                .get("behaviorConfig")
                .cloned()
                .unwrap_or(Value::Null);
        }
        self.refresh_active_mapping_config();
        self.refresh_active_interpretation_profile();
        self.engine
            .set_interpretation_profile(self.interpretation_profile.clone());
        self.menu.state = Default::default();
        self.menu.rebuild(self.menu_config());
        Ok(())
    }

    fn apply_touch_fx_payload(&mut self, touch_fx: &Value) {
        self.dance_fx_selected = sanitize_dance_fx_config(
            &touch_fx
                .get("selected")
                .cloned()
                .unwrap_or_else(default_dance_fx_selected),
        );
        self.dance_fx_assignments = touch_fx
            .get("assignments")
            .and_then(Value::as_array)
            .map(|assignments| {
                assignments
                    .iter()
                    .filter_map(|assignment| {
                        let x = assignment.get("x")?.as_u64()? as usize;
                        let y = assignment.get("y")?.as_u64()? as usize;
                        if x >= GRID_WIDTH || y >= GRID_HEIGHT {
                            return None;
                        }
                        Some(NativeDanceFxAssignment {
                            x,
                            y,
                            config: sanitize_dance_fx_config(
                                &assignment
                                    .get("config")
                                    .cloned()
                                    .unwrap_or_else(default_dance_fx_selected),
                            ),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.dance_fx_assign = None;
    }

    fn open_contextual_help(&mut self) {
        let Some(target) = self.menu.current_help_target() else {
            self.toast = Some(NativeToast {
                message: "Missing help target".into(),
                offset: 0,
            });
            return;
        };
        let Some(help) = crate::native_help::resolve_native_help(&target) else {
            self.toast = Some(NativeToast {
                message: format!("Missing help: {}", target.label),
                offset: 0,
            });
            return;
        };
        let title = format!("Help: {}", help.title);
        self.help_popup = Some(NativeHelpPopup {
            title,
            lines: wrap_help_text(&help.detail, 28),
            scroll: 0,
        });
    }

    fn turn_help_popup(&mut self, delta: i8) {
        let Some(help) = &mut self.help_popup else {
            return;
        };
        let max_scroll = help.lines.len().saturating_sub(OLED_BODY_ROWS - 1);
        let next = (help.scroll as isize + delta as isize).clamp(0, max_scroll as isize) as usize;
        help.scroll = next;
    }

    fn handle_device_input(&mut self, input: DeviceInput) -> Result<Vec<RunnerMessage>, String> {
        match input {
            DeviceInput::GridPress { x, y } => {
                if self.dance_fx_assign.is_some() {
                    self.handle_dance_fx_assignment_grid_press(x, y);
                } else if self.sample_assign.is_some() {
                    self.handle_sample_assignment_grid_press(x, y);
                } else if self.trigger_probability_assign.is_some() {
                    self.handle_trigger_probability_grid_press(x, y);
                } else if self.ui.fn_held && x == 0 && !self.ui.shift_held {
                    self.select_active_part(display_part_index_from_y(y))?;
                    self.active_dance_mode = "none".into();
                } else if self.ui.fn_held && x == GRID_WIDTH - 1 && !self.ui.shift_held {
                    self.select_dance_page_from_fn_grid(y);
                } else if self.ui.shift_held && !self.ui.fn_held && self.active_dance_mode == "none"
                {
                    if !self.handle_param_mod_grid_press(x, y) {
                        self.config_dirty = true;
                        let result =
                            self.active_engine_input_result(DeviceInput::GridPress { x, y })?;
                        return self.messages_with_input_result(result);
                    }
                } else if self.active_dance_mode == "trigger-gate" {
                    self.handle_trigger_gate_grid_press(x, y);
                } else if self.active_dance_mode == "fx" {
                    let effects = self.dance_fx_press_effects(x, y);
                    if !effects.is_empty() {
                        return self.messages_with_effects(effects);
                    }
                } else if self.active_dance_mode != "none" {
                    self.handle_dance_grid_press(x, y);
                } else {
                    self.config_dirty = true;
                    let result =
                        self.active_engine_input_result(DeviceInput::GridPress { x, y })?;
                    return self.messages_with_input_result(result);
                }
                self.messages_with_snapshot()
            }
            DeviceInput::GridRelease { x, y } => {
                if self.active_dance_mode != "none" {
                    if self.active_dance_mode == "fx" {
                        let effects = self.dance_fx_release_effects(x, y);
                        if !effects.is_empty() {
                            return self.messages_with_effects(effects);
                        }
                        return self.messages_with_snapshot();
                    }
                    if self.active_dance_mode == "xy" {
                        self.handle_dance_xy_release();
                    }
                    return self.messages_with_snapshot();
                }
                let result = self.active_engine_input_result(DeviceInput::GridRelease { x, y })?;
                self.messages_with_input_result(result)
            }
            DeviceInput::BehaviorAction(_) => {
                self.engine.on_input(input, self.bpm as f32)?;
                self.messages_with_snapshot()
            }
            DeviceInput::ButtonS { pressed } => {
                if pressed.unwrap_or(true) {
                    if let Some(effect) = self.preview_selected_sample()? {
                        return self.messages_with_effects(vec![effect]);
                    } else if self.ui.shift_held && self.sync_source == SyncSource::External {
                        self.pending_resync = true;
                    } else if self.ui.shift_held {
                        self.transport = RuntimeTransportState::Stopped;
                        self.current_ppqn_pulse = 0;
                        self.tick = 0;
                        self.transport_flash = "none";
                        self.transport_flash_pulses_remaining = 0;
                        self.event_dot_on = false;
                        self.event_dot_pulses_remaining = 0;
                        return self.messages_with_effects(vec![RuntimePlatformEffect::MidiPanic]);
                    } else if self.ui.fn_held {
                        self.toggle_active_part_trigger_gate();
                    } else {
                        self.transport = if self.transport == RuntimeTransportState::Playing {
                            RuntimeTransportState::Stopped
                        } else {
                            RuntimeTransportState::Playing
                        };
                    }
                }
                self.messages_with_snapshot()
            }
            DeviceInput::ButtonShift { pressed } => {
                self.ui.shift_held = pressed.unwrap_or(false);
                self.ui.combined_modifier_held = self.ui.shift_held && self.ui.fn_held;
                self.messages_with_snapshot()
            }
            DeviceInput::ButtonFn { pressed } => {
                self.ui.fn_held = pressed.unwrap_or(false);
                self.ui.combined_modifier_held = self.ui.shift_held && self.ui.fn_held;
                self.messages_with_snapshot()
            }
            DeviceInput::ButtonCombinedModifier { pressed } => {
                self.ui.combined_modifier_held = pressed.unwrap_or(false);
                self.messages_with_snapshot()
            }
            DeviceInput::EncoderTurn { delta, id } => {
                if let Some(index) = Self::aux_index(id.as_deref()) {
                    self.handle_aux_turn(index, delta)?;
                } else if id.as_deref().unwrap_or("main") == "main" && delta != 0 {
                    if self.help_popup.is_some() {
                        self.turn_help_popup(delta);
                    } else {
                        self.menu.turn(delta);
                        self.apply_menu_state()?;
                    }
                }
                self.messages_with_snapshot()
            }
            DeviceInput::EncoderPress { id } => {
                if let Some(index) = Self::aux_index(id.as_deref()) {
                    if let Some(effect) = self.handle_aux_press(index)? {
                        return self.messages_with_effects(vec![effect]);
                    }
                } else if id.as_deref().unwrap_or("main") == "main" {
                    if self.help_popup.is_some() {
                        self.help_popup = None;
                        return self.messages_with_snapshot();
                    }
                    if self.ui.fn_held && self.ui.shift_held {
                        self.open_contextual_help();
                        return self.messages_with_snapshot();
                    }
                    let stack_depth_before = self.menu.state.stack.len();
                    let selected_root_label = if self.menu.state.stack.is_empty() {
                        self.menu.current_label().map(str::to_string)
                    } else {
                        None
                    };
                    let selected_nested_label = if self.menu.state.stack.len() == 1 {
                        self.menu.current_label().map(str::to_string)
                    } else {
                        None
                    };
                    let selected_group_label = self.menu.current_label().map(str::to_string);
                    let selected_group_key = self.menu.current_key().map(str::to_string);
                    let entering_group = !self.menu.state.editing
                        && self.menu.current_label().is_some()
                        && self.menu.snapshot().selected_action.is_none();
                    let mut action_executed = false;
                    let mut effects = Vec::new();
                    if let Some(action) = self.menu.press() {
                        if let Some(effect) = self.execute_menu_action(action)? {
                            effects.push(effect);
                        }
                        action_executed = true;
                    } else if entering_group {
                        self.enter_root_group(selected_root_label.as_deref());
                        self.enter_nested_group(
                            stack_depth_before,
                            selected_nested_label.as_deref(),
                        )?;
                        match selected_group_label.as_deref() {
                            Some("MIDI Out") => {
                                effects.push(RuntimePlatformEffect::MidiListOutputsRequest)
                            }
                            Some("MIDI In") => {
                                effects.push(RuntimePlatformEffect::MidiListInputsRequest)
                            }
                            _ => {}
                        }
                        if let Some(key) = selected_group_key.as_deref() {
                            if let Some(effect) = self.sample_open_effect_for_key(key) {
                                effects.push(effect);
                            }
                        } else if let Some(effect) = self.sample_open_effect_for_current_group() {
                            effects.push(effect);
                        }
                    }
                    if !action_executed {
                        self.apply_menu_state()?;
                    }
                    if !effects.is_empty() {
                        return self.messages_with_effects(effects);
                    }
                }
                self.messages_with_snapshot()
            }
            DeviceInput::ButtonA { .. } => {
                if self.dance_fx_assign.is_some() {
                    self.dance_fx_assign = None;
                } else if self.sample_assign.is_some() {
                    self.sample_assign = None;
                } else if self.trigger_probability_assign.is_some() {
                    self.trigger_probability_assign = None;
                } else if self.help_popup.is_some() {
                    self.help_popup = None;
                } else if self.ui.shift_held && self.menu.delete_text_char() {
                    self.apply_menu_state()?;
                } else if self.ui.shift_held {
                    self.rebuild_engine(self.behavior)?;
                } else {
                    if self.active_dance_mode != "none" {
                        self.active_dance_mode = "none".into();
                    }
                    self.menu.back();
                }
                self.messages_with_snapshot()
            }
            DeviceInput::Other => self.messages_with_snapshot(),
        }
    }

    fn advance_algorithm(
        &mut self,
        pulses: u32,
    ) -> Result<Vec<platform_core::MusicalEvent>, String> {
        if pulses == 0 || self.transport != RuntimeTransportState::Playing {
            return Ok(Vec::new());
        }

        let mut events = Vec::new();
        if self.event_dot_pulses_remaining > 0 {
            self.event_dot_pulses_remaining -= 1;
        }
        self.event_dot_on = self.event_dot_pulses_remaining > 0;
        if self.transport_flash_pulses_remaining > 0 {
            self.transport_flash_pulses_remaining -= 1;
        }
        if pulses > 0 {
            let previous_pulse = self.current_ppqn_pulse.saturating_sub(u64::from(pulses));
            let current_pulse = self.current_ppqn_pulse;
            if crossed_ppqn_boundary(previous_pulse, current_pulse, 96) {
                self.transport_flash = "measure";
                self.transport_flash_pulses_remaining = 6;
            } else if crossed_ppqn_boundary(previous_pulse, current_pulse, 24) {
                self.transport_flash = "beat";
                self.transport_flash_pulses_remaining = 6;
            } else if self.transport_flash_pulses_remaining == 0 {
                self.transport_flash = "none";
            }
        }
        if self.part_pulse_accumulators.len() < GRID_HEIGHT {
            self.part_pulse_accumulators.resize(GRID_HEIGHT, 0);
        }
        for value in &mut self.part_pulse_accumulators {
            *value = value.saturating_add(pulses);
        }
        let active_step_pulses = self.step_pulses_for_part(self.active_part_index);
        while self.part_pulse_accumulators[self.active_part_index] >= active_step_pulses {
            self.part_pulse_accumulators[self.active_part_index] -= active_step_pulses;
            let tick = self.active_engine_tick_result()?;
            self.tick = self.tick.saturating_add(1);
            self.apply_runtime_modulation(&tick.mapped_intents, self.active_part_index);
            let tick_events = self.apply_sampler_assignments(
                tick.events,
                &tick.mapped_intents,
                self.active_part_index,
                tick.emitted_events.len(),
            );
            if !tick_events.is_empty() {
                self.event_dot_on = true;
                self.event_dot_pulses_remaining = 6;
            }
            events.extend(tick_events);
        }
        let instruments = self.instruments.clone();
        let inactive_configs = (0..self.part_engines.len())
            .map(|index| {
                (
                    self.interpretation_profile_for_part(index),
                    self.mapping_config_for_part(index),
                    self.step_pulses_for_part(index),
                    self.sense_parts.get(index).cloned(),
                    self.trigger_probability_maps
                        .get(index)
                        .cloned()
                        .unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        let mut rng = self.trigger_probability_rng;
        let mut inactive_modulation_updates = Vec::new();
        for (index, engine) in self.part_engines.iter_mut().enumerate() {
            if index == self.active_part_index {
                continue;
            }
            let Some(engine) = engine.as_mut() else {
                continue;
            };
            let (profile, mapping, step_pulses, sense, probability_map) = &inactive_configs[index];
            while self.part_pulse_accumulators[index] >= *step_pulses {
                self.part_pulse_accumulators[index] -= *step_pulses;
                engine.set_interpretation_profile(profile.clone());
                engine.set_mapping_config(mapping.clone());
                let tick = engine.tick_filtered(self.bpm as f32, |intent| {
                    trigger_probability_allows(sense.as_ref(), probability_map, &mut rng, intent)
                })?;
                inactive_modulation_updates.push((index, tick.mapped_intents.clone()));
                let tick_events = apply_sampler_assignments_for_instruments(
                    tick.events,
                    &tick.mapped_intents,
                    tick.emitted_events.len(),
                    &instruments,
                    sense.as_ref(),
                );
                if !tick_events.is_empty() {
                    self.event_dot_on = true;
                    self.event_dot_pulses_remaining = 6;
                }
                events.extend(tick_events);
            }
        }
        self.trigger_probability_rng = rng;
        for (index, mapped_intents) in inactive_modulation_updates {
            self.apply_runtime_modulation(&mapped_intents, index);
        }
        Ok(events)
    }

    fn apply_sampler_assignments(
        &self,
        events: Vec<MusicalEvent>,
        intents: &[CellTriggerIntent],
        _part_index: usize,
        mapped_event_offset: usize,
    ) -> Vec<MusicalEvent> {
        apply_sampler_assignments_for_instruments(
            events,
            intents,
            mapped_event_offset,
            &self.instruments,
            self.sense_parts.get(_part_index),
        )
    }

    fn step_pulses_for_part(&self, index: usize) -> u32 {
        let Some(sense) = self.sense_parts.get(index) else {
            return self.algorithm_step_pulses;
        };
        if sense.scan_mode == "scanning" {
            note_unit_to_pulses(&sense.scan_unit)
        } else {
            self.part_algorithm_step_pulses
                .get(index)
                .copied()
                .unwrap_or(DEFAULT_ALGORITHM_STEP_PULSES)
        }
    }

    fn apply_dance_overlay(&self, leds: &mut [Value]) {
        match self.active_dance_mode.as_str() {
            "mix" => {
                self.dim_leds(leds, 4);
                for x in 0..INSTRUMENT_SLOT_COUNT.min(GRID_WIDTH) {
                    let instrument = self.instruments.get(x);
                    let volume = instrument.map(|inst| inst.volume).unwrap_or(0).min(100);
                    let y =
                        ((f32::from(volume) / 100.0) * (GRID_HEIGHT - 1) as f32).round() as usize;
                    let bright = json!({ "r": 0, "g": 220, "b": 90 });
                    let dim = json!({ "r": 0, "g": 55, "b": 22 });
                    self.set_display_led(
                        leds,
                        x,
                        y,
                        if instrument.map(|inst| inst.kind.as_str()) == Some("none") {
                            dim
                        } else {
                            bright
                        },
                    );
                }
            }
            "pan" => {
                self.dim_leds(leds, 4);
                for y in 0..INSTRUMENT_SLOT_COUNT.min(GRID_HEIGHT) {
                    let Some(instrument) = self.instruments.get(y) else {
                        continue;
                    };
                    let (pan_pos, color) = self.dance_pan_target(instrument);
                    let left = pan_marker_left_cell(pan_pos);
                    let value = if instrument.kind == "none" {
                        dim_color(color, 4)
                    } else {
                        color
                    };
                    self.set_display_led(leds, left, y, value.clone());
                    self.set_display_led(leds, left + 1, y, value);
                }
            }
            "fx" => {
                self.dim_leds(leds, 4);
                for assignment in &self.dance_fx_assignments {
                    let id = dance_fx_cell_id(assignment.x, assignment.y);
                    let active = self
                        .active_dance_fx
                        .iter()
                        .any(|(active_id, _)| active_id == &id);
                    let limited = !active && self.active_dance_fx.len() >= 4;
                    let color = momentary_fx_color(dance_fx_type(&assignment.config));
                    self.set_display_led(
                        leds,
                        assignment.x,
                        assignment.y,
                        if active {
                            add_dim_white_overlay(&color, 70)
                        } else if limited {
                            dim_color(color, 5)
                        } else {
                            color
                        },
                    );
                }
            }
            "trigger-gate" => {
                self.dim_leds(leds, 4);
                for (row, mode) in self.trigger_gate_modes.iter().enumerate().take(GRID_HEIGHT) {
                    for (x, candidate) in [(0, "zero"), (1, "custom"), (2, "full")] {
                        let color = trigger_gate_color(candidate);
                        self.set_display_led(
                            leds,
                            x,
                            row,
                            if mode == candidate {
                                color
                            } else {
                                dim_color(color, 4)
                            },
                        );
                    }
                }
                self.set_display_led(leds, 5, 0, trigger_gate_color("zero"));
                self.set_display_led(leds, 6, 0, trigger_gate_color("custom"));
                self.set_display_led(leds, 7, 0, trigger_gate_color("full"));
            }
            "xy" => {
                self.dim_leds(leds, 4);
                self.set_display_led(leds, 4, 4, json!({ "r": 180, "g": 180, "b": 180 }));
            }
            _ => {}
        }
    }

    fn apply_sample_assignment_overlay(&self, leds: &mut [Value]) {
        let Some((instrument_slot, selected_sample_slot)) = self.sample_assign else {
            return;
        };
        self.fill_leds(leds, json!({ "r": 0, "g": 0, "b": 0 }));
        let Some(instrument) = self.instruments.get(instrument_slot) else {
            return;
        };
        for assignment in &instrument.sample_assignments {
            let color = if assignment.sample_slot == selected_sample_slot {
                match assignment.level.as_deref() {
                    Some("high") => json!({ "r": 220, "g": 0, "b": 0 }),
                    Some("medium") => json!({ "r": 220, "g": 180, "b": 0 }),
                    Some("low") => json!({ "r": 0, "g": 220, "b": 0 }),
                    _ => json!({ "r": 220, "g": 220, "b": 220 }),
                }
            } else {
                json!({ "r": 70, "g": 70, "b": 70 })
            };
            self.set_display_led(leds, assignment.x, assignment.y, color);
        }
    }

    fn apply_trigger_probability_overlay(&self, leds: &mut [Value]) {
        let Some(part_index) = self.trigger_probability_assign else {
            return;
        };
        self.fill_leds(leds, json!({ "r": 0, "g": 0, "b": 0 }));
        let Some(map) = self.trigger_probability_maps.get(part_index) else {
            return;
        };
        for y in 0..GRID_HEIGHT {
            for x in 0..GRID_WIDTH {
                let color = match map.get(y * GRID_WIDTH + x).map(String::as_str) {
                    Some("low") => json!({ "r": 220, "g": 0, "b": 0 }),
                    Some("high") => json!({ "r": 220, "g": 180, "b": 0 }),
                    Some("full") => json!({ "r": 0, "g": 220, "b": 0 }),
                    _ => json!({ "r": 0, "g": 0, "b": 0 }),
                };
                self.set_display_led(leds, x, y, color);
            }
        }
    }

    fn apply_param_mod_overlay(&self, leds: &mut [Value]) {
        if !self.ui.shift_held
            || self.ui.fn_held
            || self.active_dance_mode != "none"
            || self.sample_assign.is_some()
            || self.trigger_probability_assign.is_some()
            || self.dance_fx_assign.is_some()
        {
            return;
        }
        let Some(mut highlighted) = self
            .menu
            .current_param_binding()
            .map(native_binding_from_spec)
        else {
            return;
        };
        if let Some(field) = highlighted.key.strip_prefix("behavior.") {
            highlighted.key = format!("parts.{}.l1.behaviorConfig.{field}", self.active_part_index);
        }
        let Some(param_mods) = self.param_mods.get(self.active_part_index) else {
            return;
        };
        let lane = json!({ "r": 18, "g": 18, "b": 24 });
        for x in 0..GRID_WIDTH {
            self.set_display_led(leds, x, 0, lane.clone());
            self.set_display_led(leds, x, 1, lane.clone());
        }
        for y in 0..GRID_HEIGHT {
            self.set_display_led(leds, 0, y, lane.clone());
            self.set_display_led(leds, 1, y, lane.clone());
        }
        if let Some(binding) = param_mods.x.first().and_then(Option::as_ref) {
            self.paint_param_mod_axis_slot(leds, binding, &highlighted.key, "x", 0);
        }
        if let Some(binding) = param_mods.x.get(1).and_then(Option::as_ref) {
            self.paint_param_mod_axis_slot(leds, binding, &highlighted.key, "x", 1);
        }
        if let Some(binding) = param_mods.y.first().and_then(Option::as_ref) {
            self.paint_param_mod_axis_slot(leds, binding, &highlighted.key, "y", 0);
        }
        if let Some(binding) = param_mods.y.get(1).and_then(Option::as_ref) {
            self.paint_param_mod_axis_slot(leds, binding, &highlighted.key, "y", 1);
        }
        self.set_display_led(leds, 0, 0, json!({ "r": 255, "g": 255, "b": 255 }));
        self.set_display_led(leds, 1, 1, json!({ "r": 255, "g": 255, "b": 255 }));
    }

    fn paint_param_mod_axis_slot(
        &self,
        leds: &mut [Value],
        binding: &NativeParamBinding,
        highlighted_key: &str,
        axis: &str,
        slot: usize,
    ) {
        let color = if binding.invert {
            json!({ "r": 255, "g": 0, "b": 90 })
        } else {
            json!({ "r": 0, "g": 255, "b": 120 })
        };
        let color = if binding.key == highlighted_key {
            color
        } else {
            dim_color(color, 3)
        };
        if axis == "x" {
            for x in 0..GRID_WIDTH {
                self.set_display_led(leds, x, slot, color.clone());
            }
        } else {
            for y in 0..GRID_HEIGHT {
                self.set_display_led(leds, slot, y, color.clone());
            }
        }
    }

    fn apply_scan_progress_overlay(&self, leds: &mut [Value]) {
        let Some(sense) = self.sense_parts.get(self.active_part_index) else {
            return;
        };
        if sense.scan_mode != "scanning" {
            return;
        }
        let reverse = sense.scan_direction == "reverse";
        if sense.scan_axis == "columns" {
            let sections = scan_section_count(sense.scan_sections, GRID_HEIGHT);
            if sections > 1 {
                let section_height = (GRID_HEIGHT / sections).max(1);
                let step =
                    scan_index_for_overlay(self.tick as usize, GRID_WIDTH * sections, reverse);
                let section = step / GRID_WIDTH;
                let x = step % GRID_WIDTH;
                let first_y = section * section_height;
                for dy in 0..section_height {
                    self.add_scan_overlay_led(leds, x, first_y + dy);
                }
            } else {
                let x = scan_index_for_overlay(self.tick as usize, GRID_WIDTH, reverse);
                for y in 0..GRID_HEIGHT {
                    self.add_scan_overlay_led(leds, x, y);
                }
            }
        } else {
            let sections = scan_section_count(sense.scan_sections, GRID_WIDTH);
            if sections > 1 {
                let section_width = (GRID_WIDTH / sections).max(1);
                let step =
                    scan_index_for_overlay(self.tick as usize, GRID_HEIGHT * sections, reverse);
                let section = step / GRID_HEIGHT;
                let y = step % GRID_HEIGHT;
                let first_x = section * section_width;
                for dx in 0..section_width {
                    self.add_scan_overlay_led(leds, first_x + dx, y);
                }
            } else {
                let y = scan_index_for_overlay(self.tick as usize, GRID_HEIGHT, reverse);
                for x in 0..GRID_WIDTH {
                    self.add_scan_overlay_led(leds, x, y);
                }
            }
        }
    }

    fn add_scan_overlay_led(&self, leds: &mut [Value], x: usize, y: usize) {
        if x >= GRID_WIDTH || y >= GRID_HEIGHT {
            return;
        }
        let index = display_index(x, y);
        if let Some(cell) = leds.get_mut(index) {
            *cell = add_dim_white_overlay(cell, 24);
        }
    }

    fn set_display_led(&self, leds: &mut [Value], x: usize, y: usize, value: Value) {
        if x >= GRID_WIDTH || y >= GRID_HEIGHT {
            return;
        }
        let index = display_index(x, y);
        if let Some(cell) = leds.get_mut(index) {
            *cell = value;
        }
    }

    fn fill_leds(&self, leds: &mut [Value], value: Value) {
        for cell in leds.iter_mut() {
            *cell = value.clone();
        }
    }

    fn dim_leds(&self, leds: &mut [Value], divisor: i64) {
        for cell in leds.iter_mut() {
            *cell = dim_color(cell.clone(), divisor);
        }
    }

    fn dance_pan_target(&self, instrument: &NativeInstrumentSlot) -> (u8, Value) {
        if let Some(bus_index) = instrument
            .route
            .strip_prefix("fx_bus_")
            .and_then(|value| value.parse::<usize>().ok())
            .and_then(|value| value.checked_sub(1))
        {
            let pan = self
                .fx_buses
                .get(bus_index)
                .map(|bus| bus.pan_pos)
                .unwrap_or(instrument.pan_pos);
            let color = match bus_index {
                0 => json!({ "r": 190, "g": 80, "b": 255 }),
                1 => json!({ "r": 0, "g": 210, "b": 255 }),
                2 => json!({ "r": 0, "g": 230, "b": 120 }),
                3 => json!({ "r": 255, "g": 160, "b": 0 }),
                _ => json!({ "r": 255, "g": 255, "b": 255 }),
            };
            return (pan, color);
        }
        (instrument.pan_pos, json!({ "r": 255, "g": 255, "b": 255 }))
    }

    fn dance_fx_start_effect_for_assignment(
        &self,
        assignment: &NativeDanceFxAssignment,
    ) -> Option<RuntimePlatformEffect> {
        let x = assignment.x;
        let y = assignment.y;
        let fx_type = dance_fx_type(&assignment.config).to_string();
        if fx_type == "none" {
            return None;
        }
        Some(RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::MomentaryFxStart {
                id: dance_fx_cell_id(x, y),
                fx_type,
                params: dance_fx_params(&assignment.config),
                target: momentary_fx_target(dance_fx_target_key(&assignment.config)),
            },
        })
    }

    fn dance_fx_press_effects(&mut self, x: usize, y: usize) -> Vec<RuntimePlatformEffect> {
        let Some(assignment) = self.dance_fx_assignment_at(x, y).cloned() else {
            return Vec::new();
        };
        let fx_type = dance_fx_type(&assignment.config).to_string();
        if fx_type == "none" {
            return Vec::new();
        }
        let id = dance_fx_cell_id(x, y);
        if self
            .active_dance_fx
            .iter()
            .any(|(active_id, _)| active_id == &id)
        {
            return Vec::new();
        }
        let mut effects = Vec::new();
        if let Some(index) = self
            .active_dance_fx
            .iter()
            .position(|(_, active_type)| active_type == &fx_type)
        {
            let (old_id, _) = self.active_dance_fx.remove(index);
            effects.push(RuntimePlatformEffect::AudioCommand {
                command: RuntimeAudioCommand::MomentaryFxStop { id: old_id },
            });
        } else if self.active_dance_fx.len() >= 4 {
            return Vec::new();
        }
        if let Some(start) = self.dance_fx_start_effect_for_assignment(&assignment) {
            self.active_dance_fx.push((id, fx_type));
            effects.push(start);
        }
        effects
    }

    fn dance_fx_release_effects(&mut self, x: usize, y: usize) -> Vec<RuntimePlatformEffect> {
        let id = dance_fx_cell_id(x, y);
        let Some(index) = self
            .active_dance_fx
            .iter()
            .position(|(active_id, _)| active_id == &id)
        else {
            return Vec::new();
        };
        let (id, _) = self.active_dance_fx.remove(index);
        vec![RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::MomentaryFxStop { id },
        }]
    }

    fn dance_fx_assignment_at(&self, x: usize, y: usize) -> Option<&NativeDanceFxAssignment> {
        self.dance_fx_assignments
            .iter()
            .find(|assignment| assignment.x == x && assignment.y == y)
    }

    fn handle_dance_fx_assignment_grid_press(&mut self, x: usize, y: usize) {
        let Some(config) = self.dance_fx_assign.take() else {
            return;
        };
        let same_existing = self.dance_fx_assignments.iter().any(|assignment| {
            assignment.x == x && assignment.y == y && assignment.config == config
        });
        self.dance_fx_assignments.retain(|assignment| {
            assignment.x != x || assignment.y != y || assignment.config != config
        });
        if same_existing {
            self.config_dirty = true;
            self.toast = Some(NativeToast {
                message: "FX cleared".into(),
                offset: 0,
            });
            return;
        }
        self.dance_fx_assignments
            .retain(|assignment| assignment.x != x || assignment.y != y);
        if dance_fx_type(&config) != "none" {
            self.dance_fx_assignments
                .push(NativeDanceFxAssignment { x, y, config });
        }
        self.config_dirty = true;
        self.toast = Some(NativeToast {
            message: "FX mapped".into(),
            offset: 0,
        });
    }

    fn handle_dance_grid_press(&mut self, x: usize, y: usize) {
        match self.active_dance_mode.as_str() {
            "mix" => {
                if let Some(instrument) = self.instruments.get_mut(x) {
                    if instrument.kind != "none" {
                        let volume = ((y as f32 / (GRID_HEIGHT - 1) as f32) * 100.0).round() as u8;
                        if instrument.volume != volume {
                            instrument.volume = volume;
                            self.config_dirty = true;
                        }
                    }
                }
            }
            "pan" => {
                if let Some(instrument) = self.instruments.get_mut(y) {
                    if instrument.kind != "none" {
                        let pan_pos = touch_pan_pos_from_grid_x(x);
                        if let Some(bus_index) = instrument
                            .route
                            .strip_prefix("fx_bus_")
                            .and_then(|value| value.parse::<usize>().ok())
                            .and_then(|value| value.checked_sub(1))
                        {
                            if let Some(bus) = self.fx_buses.get_mut(bus_index) {
                                if bus.pan_pos != pan_pos {
                                    bus.pan_pos = pan_pos;
                                    self.config_dirty = true;
                                }
                            }
                        }
                        if instrument.pan_pos != pan_pos {
                            instrument.pan_pos = pan_pos;
                            self.config_dirty = true;
                        }
                    }
                }
            }
            "xy" => {
                self.handle_dance_xy_press(x, y);
            }
            _ => {}
        }
        self.menu.rebuild(self.menu_config());
    }

    fn handle_dance_xy_press(&mut self, x: usize, y: usize) {
        let mut x_value = x.min(GRID_WIDTH - 1) as f32 / (GRID_WIDTH - 1) as f32;
        let mut y_value = y.min(GRID_HEIGHT - 1) as f32 / (GRID_HEIGHT - 1) as f32;
        if self.xy_invert_x {
            x_value = 1.0 - x_value;
        }
        if self.xy_invert_y {
            y_value = 1.0 - y_value;
        }
        self.xy_touch = NativeXyTouch {
            x: x_value,
            y: y_value,
            active: true,
        };
        self.config_dirty = true;
    }

    fn handle_dance_xy_release(&mut self) {
        if self.xy_release == "reset-center" {
            self.xy_touch = NativeXyTouch {
                x: 0.5,
                y: 0.5,
                active: false,
            };
        } else {
            self.xy_touch.active = false;
        }
        self.config_dirty = true;
    }

    fn apply_runtime_modulation(&mut self, intents: &[CellTriggerIntent], part_index: usize) {
        let intent = intents
            .iter()
            .find(|intent| {
                matches!(
                    intent.kind,
                    platform_core::CellTriggerKind::Activate
                        | platform_core::CellTriggerKind::Scanned
                        | platform_core::CellTriggerKind::Stable
                )
            })
            .or_else(|| intents.last());
        if let Some(intent) = intent {
            if let Some(param_mods) = self.param_mods.get(part_index).cloned() {
                for binding in param_mods.x.iter().flatten() {
                    let value = quantize_binding_value(
                        axis_norm(intent.x, GRID_WIDTH, binding.invert),
                        binding,
                    );
                    self.apply_param_binding_value(&binding.key, value);
                }
                for binding in param_mods.y.iter().flatten() {
                    let value = quantize_binding_value(
                        axis_norm(intent.y, GRID_HEIGHT, binding.invert),
                        binding,
                    );
                    self.apply_param_binding_value(&binding.key, value);
                }
            }
        }
        self.apply_xy_modulation();
    }

    fn apply_xy_modulation(&mut self) {
        if !self.xy_touch.active && self.xy_release != "sample-hold" {
            return;
        }
        if let Some(binding) = self.xy_x_binding.clone() {
            let norm = if self.xy_invert_x {
                1.0 - self.xy_touch.x
            } else {
                self.xy_touch.x
            };
            let value = quantize_binding_value(norm, &binding);
            self.apply_param_binding_value(&binding.key, value);
        }
        if let Some(binding) = self.xy_y_binding.clone() {
            let norm = if self.xy_invert_y {
                1.0 - self.xy_touch.y
            } else {
                self.xy_touch.y
            };
            let value = quantize_binding_value(norm, &binding);
            self.apply_param_binding_value(&binding.key, value);
        }
    }

    fn apply_param_binding_value(&mut self, key: &str, value: Value) {
        match key {
            "sound.noteLengthMs" => {
                if let Some(value) = value.as_f64() {
                    self.global_sound.note_length_ms = value.round().clamp(30.0, 2000.0) as u32;
                    self.config_dirty = true;
                }
            }
            "sound.velocityScalePct" => {
                if let Some(value) = value.as_f64() {
                    self.global_sound.velocity_scale_pct = value.round().clamp(0.0, 200.0) as u16;
                    self.config_dirty = true;
                }
            }
            "sound.voiceStealingMode" => {
                if let Some(value) = value.as_str() {
                    if matches!(value, "off" | "lenient" | "balanced" | "aggressive") {
                        self.voice_stealing_mode = value.into();
                        self.config_dirty = true;
                    }
                }
            }
            _ => {
                if let Some((index, field)) = parse_part_behavior_config_binding_key(key) {
                    if let Some(config) = self.part_behavior_configs.get_mut(index) {
                        let mut object = config.as_object().cloned().unwrap_or_default();
                        object.insert(field.into(), value.clone());
                        *config = Value::Object(object.clone());
                        if index == self.active_part_index {
                            self.behavior_config = Value::Object(object);
                        }
                        self.config_dirty = true;
                    }
                } else if let Some((index, field)) = parse_instrument_binding_key(key) {
                    if let Some(instrument) = self.instruments.get_mut(index) {
                        apply_instrument_binding_value(
                            instrument,
                            field,
                            value,
                            &mut self.config_dirty,
                        );
                    }
                }
            }
        }
    }

    fn apply_fn_overlay(&self, leds: &mut [Value]) {
        if !self.ui.fn_held {
            return;
        }

        for cell in leds.iter_mut() {
            if let Some(object) = cell.as_object_mut() {
                let r = object.get("r").and_then(|v| v.as_i64()).unwrap_or(0);
                let g = object.get("g").and_then(|v| v.as_i64()).unwrap_or(0);
                let b = object.get("b").and_then(|v| v.as_i64()).unwrap_or(0);
                object.insert("r".into(), Value::from(r / 4));
                object.insert("g".into(), Value::from(g / 4));
                object.insert("b".into(), Value::from(b / 4));
            }
        }

        for row in 0..GRID_HEIGHT {
            let configured = self
                .part_behavior_ids
                .get(row)
                .map(|id| id != "none")
                .unwrap_or(false);
            let color = if self.active_dance_mode != "none" {
                if configured {
                    json!({ "r": 0, "g": 191, "b": 95 })
                } else {
                    json!({ "r": 2, "g": 2, "b": 3 })
                }
            } else if row == self.active_part_index {
                json!({ "r": 0, "g": 191, "b": 191 })
            } else if configured {
                json!({ "r": 0, "g": 120, "b": 0 })
            } else {
                json!({ "r": 0, "g": 48, "b": 23 })
            };
            self.set_display_led(leds, 0, row, color);
        }

        let page_options = ["mix", "pan", "fx", "trigger-gate", "xy"];
        for (row, mode) in page_options.iter().enumerate() {
            let selected = self.active_dance_mode == *mode || self.dance_mode == *mode;
            let color = if selected {
                json!({ "r": 0, "g": 158, "b": 158 })
            } else {
                json!({ "r": 0, "g": 60, "b": 60 })
            };
            self.set_display_led(leds, GRID_WIDTH - 1, row, color);
        }
    }

    fn handle_trigger_gate_grid_press(&mut self, x: usize, y: usize) {
        let mode = match x {
            0 => Some("zero"),
            1 => Some("custom"),
            2 => Some("full"),
            6 => Some("custom"),
            7 => Some("full"),
            _ => None,
        };
        let Some(mode) = mode else {
            return;
        };

        if x == 6 && y == 0 {
            for part_mode in &mut self.trigger_gate_modes {
                *part_mode = mode.into();
            }
            for part in &mut self.sense_parts {
                part.trigger_probability_mode = mode.into();
            }
            for restore in &mut self.trigger_gate_restore_modes {
                *restore = None;
            }
            if let Err(error) = self.activate_engine(self.active_part_index) {
                self.toast = Some(NativeToast {
                    message: error,
                    offset: 0,
                });
            }
            return;
        }

        if let Some(part_mode) = self.trigger_gate_modes.get_mut(y) {
            *part_mode = mode.into();
        }
        if let Some(part) = self.sense_parts.get_mut(y) {
            part.trigger_probability_mode = mode.into();
        }
        if let Some(restore) = self.trigger_gate_restore_modes.get_mut(y) {
            *restore = None;
        }
        if y == self.active_part_index {
            if let Err(error) = self.activate_engine(y) {
                self.toast = Some(NativeToast {
                    message: error,
                    offset: 0,
                });
            }
        }
    }

    fn select_active_part(&mut self, index: usize) -> Result<(), String> {
        let index = index.min(GRID_HEIGHT.saturating_sub(1));
        if index == self.active_part_index {
            return Ok(());
        }
        self.store_active_engine();
        self.active_part_index = index;
        self.algorithm_step_pulses = self
            .part_algorithm_step_pulses
            .get(index)
            .copied()
            .unwrap_or(DEFAULT_ALGORITHM_STEP_PULSES);
        self.activate_engine(index)?;
        self.menu.rebuild(self.menu_config());
        Ok(())
    }

    fn toggle_active_part_trigger_gate(&mut self) {
        let current = self
            .sense_parts
            .get(self.active_part_index)
            .map(|part| part.trigger_probability_mode.clone())
            .or_else(|| self.trigger_gate_modes.get(self.active_part_index).cloned())
            .unwrap_or_else(|| "full".into());
        if current == "zero" {
            let restore = self
                .trigger_gate_restore_modes
                .get(self.active_part_index)
                .and_then(Clone::clone)
                .unwrap_or_else(|| "full".into());
            if let Some(mode) = self.trigger_gate_modes.get_mut(self.active_part_index) {
                *mode = restore.clone();
            }
            if let Some(part) = self.sense_parts.get_mut(self.active_part_index) {
                part.trigger_probability_mode = restore.clone();
            }
            if let Some(slot) = self
                .trigger_gate_restore_modes
                .get_mut(self.active_part_index)
            {
                *slot = None;
            }
            self.toast = Some(NativeToast {
                message: format!("P{} triggers {}", self.active_part_index + 1, restore),
                offset: 0,
            });
        } else {
            if let Some(slot) = self
                .trigger_gate_restore_modes
                .get_mut(self.active_part_index)
            {
                *slot = Some(current);
            }
            if let Some(mode) = self.trigger_gate_modes.get_mut(self.active_part_index) {
                *mode = "zero".into();
            }
            if let Some(part) = self.sense_parts.get_mut(self.active_part_index) {
                part.trigger_probability_mode = "zero".into();
            }
            self.toast = Some(NativeToast {
                message: format!("P{} triggers off", self.active_part_index + 1),
                offset: 0,
            });
        }
        if let Err(error) = self.activate_engine(self.active_part_index) {
            self.toast = Some(NativeToast {
                message: error,
                offset: 0,
            });
        }
        self.menu.rebuild(self.menu_config());
    }

    fn select_dance_page_from_fn_grid(&mut self, y: usize) {
        let next_mode = match y {
            0 => Some("mix"),
            1 => Some("pan"),
            2 => Some("fx"),
            3 => Some("trigger-gate"),
            4 => Some("xy"),
            _ => None,
        };
        let Some(next_mode) = next_mode else {
            return;
        };
        self.dance_mode = next_mode.into();
        self.active_dance_mode = self.dance_mode.clone();
        self.menu.state.stack = vec![3];
        self.menu.state.cursor = 0;
        self.menu.state.editing = false;
        self.menu.rebuild(self.menu_config());
    }
}

fn derive_instrument_name(_index: usize, kind: &str) -> String {
    kind.to_string()
}

fn derive_bus_name(bus: &NativeFxBus) -> String {
    match (bus.slot1_type.as_str(), bus.slot2_type.as_str()) {
        ("none", "none") => "(none)".into(),
        ("none", slot) => slot.into(),
        (slot, "none") => slot.into(),
        (slot1, slot2) => format!("{slot1}+{slot2}"),
    }
}

fn default_instruments() -> Vec<NativeInstrumentSlot> {
    (0..INSTRUMENT_SLOT_COUNT)
        .map(NativeInstrumentSlot::new)
        .collect()
}

fn default_sense_parts() -> Vec<NativeSensePart> {
    let mut parts = vec![NativeSensePart::default(); GRID_HEIGHT];
    for part in parts.iter_mut().skip(1) {
        part.event_enabled = false;
    }
    parts
}

fn default_fx_buses() -> Vec<NativeFxBus> {
    vec![NativeFxBus::default(); 4]
}

fn default_global_fx_slots() -> Vec<String> {
    vec!["none".into(); 2]
}

fn fx_slot_payload(slot_type: &str) -> Value {
    json!({ "type": slot_type, "params": fx_default_params(slot_type) })
}

fn fx_default_params(slot_type: &str) -> Value {
    match slot_type {
        "delay" => json!({ "timeMs": 250, "feedbackPct": 25, "mixPct": 20 }),
        "duck" => json!({ "amountPct": 50, "releaseMs": 180, "source": "kick" }),
        "reverb" => json!({ "roomPct": 45, "mixPct": 20 }),
        "chorus" => json!({ "rateHz": 1, "depthPct": 35, "mixPct": 25 }),
        "compressor" => json!({ "thresholdDb": -12, "ratio": 3, "makeupDb": 0 }),
        "limiter" => json!({ "ceilingDb": -1, "releaseMs": 100 }),
        _ => json!({}),
    }
}

fn default_dance_fx_selected() -> Value {
    json!({
        "fxType": "none",
        "targetKey": "master",
        "params": {}
    })
}

fn sanitize_dance_fx_config(config: &Value) -> Value {
    let fx_type = match dance_fx_type(config) {
        "stutter" | "freeze" | "filter_sweep" | "pitch_shift" => dance_fx_type(config),
        _ => "none",
    };
    let target_key = match dance_fx_target_key(config) {
        "master" | "fx_bus_1" | "fx_bus_2" | "instrument_1" | "instrument_2" | "instrument_3"
        | "instrument_4" | "instrument_5" | "instrument_6" | "instrument_7" | "instrument_8" => {
            dance_fx_target_key(config)
        }
        _ => "master",
    };
    let mut params = serde_json::Map::new();
    for key in dance_fx_param_keys(fx_type) {
        let value = config
            .get("params")
            .and_then(|params| params.get(*key))
            .and_then(Value::as_i64)
            .unwrap_or_else(|| i64::from(dance_fx_param_default(fx_type, key)));
        params.insert(
            (*key).into(),
            json!(sanitize_dance_fx_param(fx_type, key, value)),
        );
    }
    json!({ "fxType": fx_type, "targetKey": target_key, "params": params })
}

fn sanitize_dance_fx_param(fx_type: &str, key: &str, value: i64) -> i64 {
    match (fx_type, key) {
        ("stutter", "rateHz") => value.clamp(1, 32),
        ("stutter", "depthPct") => value.clamp(0, 100),
        ("freeze", "releaseMs") => value.clamp(10, 5000),
        ("freeze", "mixPct") => value.clamp(0, 100),
        ("filter_sweep", "cutoffPct") => value.clamp(0, 100),
        ("filter_sweep", "resonancePct") => value.clamp(0, 100),
        ("filter_sweep", "sweepInMs") => value.clamp(10, 3000),
        ("filter_sweep", "sweepOutMs") => value.clamp(10, 3000),
        ("pitch_shift", "semitones") => value.clamp(-24, 24),
        ("pitch_shift", "cents") => value.clamp(-100, 100),
        ("pitch_shift", "mixPct") => value.clamp(0, 100),
        _ => value,
    }
}

fn dance_fx_type(config: &Value) -> &str {
    config
        .get("fxType")
        .and_then(Value::as_str)
        .unwrap_or("none")
}

fn dance_fx_target_key(config: &Value) -> &str {
    config
        .get("targetKey")
        .and_then(Value::as_str)
        .unwrap_or("master")
}

fn dance_fx_params_map(config: &Value) -> serde_json::Map<String, Value> {
    config
        .get("params")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn dance_fx_params(config: &Value) -> BTreeMap<String, Value> {
    dance_fx_params_map(config).into_iter().collect()
}

fn dance_fx_param_keys(fx_type: &str) -> &'static [&'static str] {
    match fx_type {
        "stutter" => &["rateHz", "depthPct"],
        "freeze" => &["releaseMs", "mixPct"],
        "filter_sweep" => &["cutoffPct", "resonancePct", "sweepInMs", "sweepOutMs"],
        "pitch_shift" => &["semitones", "cents", "mixPct"],
        _ => &[],
    }
}

fn dance_fx_param_default(fx_type: &str, key: &str) -> i32 {
    match (fx_type, key) {
        ("stutter", "rateHz") => 8,
        ("stutter", "depthPct") => 100,
        ("freeze", "releaseMs") => 500,
        ("freeze", "mixPct") => 100,
        ("filter_sweep", "cutoffPct") => 50,
        ("filter_sweep", "resonancePct") => 0,
        ("filter_sweep", "sweepInMs") => 120,
        ("filter_sweep", "sweepOutMs") => 180,
        ("pitch_shift", "semitones") => 0,
        ("pitch_shift", "cents") => 0,
        ("pitch_shift", "mixPct") => 100,
        _ => 0,
    }
}

fn momentary_fx_color(fx_type: &str) -> Value {
    match fx_type {
        "stutter" => json!({ "r": 255, "g": 170, "b": 40 }),
        "freeze" => json!({ "r": 80, "g": 210, "b": 255 }),
        "filter_sweep" => json!({ "r": 80, "g": 255, "b": 120 }),
        "pitch_shift" => json!({ "r": 190, "g": 90, "b": 255 }),
        _ => json!({ "r": 40, "g": 40, "b": 40 }),
    }
}

fn apply_sampler_assignments_for_instruments(
    events: Vec<MusicalEvent>,
    intents: &[CellTriggerIntent],
    mapped_event_offset: usize,
    instruments: &[NativeInstrumentSlot],
    sense: Option<&NativeSensePart>,
) -> Vec<MusicalEvent> {
    let mut out = Vec::with_capacity(events.len());
    for event in events.iter().take(mapped_event_offset) {
        out.push(event.clone());
    }
    for (intent_index, event) in events.iter().skip(mapped_event_offset).enumerate() {
        let Some(intent) = intents.get(intent_index) else {
            out.push(event.clone());
            continue;
        };
        let channel = match event {
            MusicalEvent::NoteOn { channel, .. } | MusicalEvent::NoteOff { channel, .. } => {
                *channel
            }
            MusicalEvent::Cc { channel, .. } => *channel,
        };
        if let Some(sense) = sense {
            out.extend(cc_events_from_intent(intent, sense, channel));
        }
        let mut event = event.clone();
        let mut suppress = false;
        match &mut event {
            MusicalEvent::NoteOn {
                channel,
                note,
                velocity,
                ..
            } => {
                if let Some(sense_velocity) =
                    sense.and_then(|sense| velocity_from_intent(intent, sense))
                {
                    *velocity = sense_velocity;
                }
                if let Some(instrument) = instruments.get(*channel as usize) {
                    if instrument.kind == "sampler" {
                        if let Some(assignment) = instrument
                            .sample_assignments
                            .iter()
                            .find(|assignment| assignment.x == intent.x && assignment.y == intent.y)
                        {
                            *note = 36 + assignment.sample_slot.min(7) as u8;
                            *velocity =
                                sampler_assignment_velocity(*velocity, assignment, instrument);
                        } else {
                            suppress = true;
                        }
                    }
                }
            }
            MusicalEvent::NoteOff { channel, note } => {
                if let Some(instrument) = instruments.get(*channel as usize) {
                    if instrument.kind == "sampler" {
                        if let Some(assignment) = instrument
                            .sample_assignments
                            .iter()
                            .find(|assignment| assignment.x == intent.x && assignment.y == intent.y)
                        {
                            *note = 36 + assignment.sample_slot.min(7) as u8;
                        } else {
                            suppress = true;
                        }
                    }
                }
            }
            MusicalEvent::Cc { .. } => {}
        }
        if !suppress {
            out.push(event);
        }
    }
    out
}

fn cc_events_from_intent(
    intent: &CellTriggerIntent,
    sense: &NativeSensePart,
    channel: u8,
) -> Vec<MusicalEvent> {
    let mut events = Vec::new();
    push_lane_cc(
        &mut events,
        &sense.x_filter_cutoff,
        intent.x,
        GRID_WIDTH,
        channel,
        74,
    );
    push_lane_cc(
        &mut events,
        &sense.y_filter_cutoff,
        intent.y,
        GRID_HEIGHT,
        channel,
        74,
    );
    push_lane_cc(
        &mut events,
        &sense.x_filter_resonance,
        intent.x,
        GRID_WIDTH,
        channel,
        71,
    );
    push_lane_cc(
        &mut events,
        &sense.y_filter_resonance,
        intent.y,
        GRID_HEIGHT,
        channel,
        71,
    );
    events
}

fn push_lane_cc(
    events: &mut Vec<MusicalEvent>,
    lane: &NativeValueLane,
    index: usize,
    size: usize,
    channel: u8,
    controller: u8,
) {
    if !lane.enabled {
        return;
    }
    events.push(MusicalEvent::Cc {
        channel: channel.min(15),
        controller,
        value: value_from_lane(index, size, lane),
    });
}

fn velocity_from_intent(intent: &CellTriggerIntent, sense: &NativeSensePart) -> Option<u8> {
    let mut values = Vec::new();
    if sense.x_velocity.enabled {
        values.push(value_from_lane(intent.x, GRID_WIDTH, &sense.x_velocity));
    }
    if sense.y_velocity.enabled {
        values.push(value_from_lane(intent.y, GRID_HEIGHT, &sense.y_velocity));
    }
    if values.is_empty() {
        return None;
    }
    Some(
        ((values.iter().map(|value| u16::from(*value)).sum::<u16>() / values.len() as u16)
            .clamp(1, 127)) as u8,
    )
}

fn value_from_lane(index: usize, size: usize, lane: &NativeValueLane) -> u8 {
    let size = size.max(1);
    let shifted = ((index as i32 + lane.grid_offset).rem_euclid(size as i32)) as f32;
    let norm = shifted / (size.saturating_sub(1).max(1) as f32);
    (f32::from(lane.from) + norm * (f32::from(lane.to) - f32::from(lane.from)))
        .round()
        .clamp(0.0, 127.0) as u8
}

fn axis_norm(index: usize, size: usize, invert: bool) -> f32 {
    let norm = index.min(size.saturating_sub(1)) as f32 / size.saturating_sub(1).max(1) as f32;
    if invert {
        1.0 - norm
    } else {
        norm
    }
}

fn param_mod_grid_targets(x: usize, y: usize) -> Vec<(&'static str, usize)> {
    if x == 0 && y == 0 {
        return vec![("x", 0), ("y", 0)];
    }
    if x == 1 && y == 1 {
        return vec![("x", 1), ("y", 1)];
    }
    let mut targets = Vec::new();
    if y == 0 || y == 1 {
        targets.push(("x", y));
    }
    if x == 0 || x == 1 {
        targets.push(("y", x));
    }
    targets
}

fn param_mod_next_toggle_mode(current: Option<&NativeParamBinding>, key: &str) -> &'static str {
    if current.map(|binding| binding.key.as_str()) != Some(key) {
        return "regular";
    }
    if current.map(|binding| binding.invert).unwrap_or(false) {
        "clear"
    } else {
        "invert"
    }
}

fn quantize_binding_value(norm: f32, binding: &NativeParamBinding) -> Value {
    let norm = norm.clamp(0.0, 1.0);
    if binding.kind == "enum" && !binding.options.is_empty() {
        let index = (norm * (binding.options.len().saturating_sub(1)) as f32).round() as usize;
        return json!(binding.options[index.min(binding.options.len() - 1)]);
    }
    if binding.kind == "bool" {
        return json!(norm >= 0.5);
    }
    let min = binding.min.unwrap_or(0.0);
    let max = binding.max.unwrap_or(127.0);
    let step = binding.step.unwrap_or(1.0);
    let raw = min + f64::from(norm) * (max - min);
    let stepped = if step > 0.0 {
        (raw / step).round() * step
    } else {
        raw
    };
    json!(stepped.clamp(min, max))
}

fn parse_instrument_binding_key(key: &str) -> Option<(usize, &str)> {
    let rest = key.strip_prefix("instruments.")?;
    let (index, field) = rest.split_once('.')?;
    Some((index.parse::<usize>().ok()?, field))
}

fn parse_part_behavior_config_binding_key(key: &str) -> Option<(usize, &str)> {
    let rest = key.strip_prefix("parts.")?;
    let (index, field) = rest.split_once(".l1.behaviorConfig.")?;
    Some((index.parse::<usize>().ok()?, field))
}

fn apply_instrument_binding_value(
    instrument: &mut NativeInstrumentSlot,
    field: &str,
    value: Value,
    config_dirty: &mut bool,
) {
    match field {
        "midi.enabled" => {
            let Some(value) = value.as_bool() else {
                return;
            };
            instrument.midi_enabled = value;
        }
        _ => {
            let Some(value) = value.as_f64() else {
                return;
            };
            match field {
                "mixer.volume" => instrument.volume = value.round().clamp(0.0, 127.0) as u8,
                "mixer.panPos" => {
                    instrument.pan_pos =
                        value.round().clamp(0.0, f64::from(PAN_POSITION_COUNT - 1)) as u8
                }
                "synth.amp.gainPct" => {
                    instrument.synth_gain_pct = value.round().clamp(0.0, 100.0) as u8
                }
                "sample.tuneSemis" => {
                    instrument.sample_tune_semis = value.round().clamp(-24.0, 24.0) as i8
                }
                "sample.amp.gainPct" => {
                    instrument.sample_gain_pct = value.round().clamp(0.0, 100.0) as u8
                }
                "sample.baseVelocity" => {
                    instrument.sample_base_velocity = value.round().clamp(1.0, 127.0) as u8
                }
                "midi.velocity" => instrument.midi_velocity = value.round().clamp(1.0, 127.0) as u8,
                "midi.durationMs" => {
                    instrument.midi_duration_ms = value.round().clamp(10.0, 5000.0) as u16
                }
                _ => return,
            }
        }
    }
    *config_dirty = true;
}

fn sampler_assignment_velocity(
    source_velocity: u8,
    assignment: &NativeSampleAssignment,
    instrument: &NativeInstrumentSlot,
) -> u8 {
    let base: u8 = match assignment.level.as_deref() {
        Some("high") => instrument.sample_velocity_high,
        Some("medium") => instrument.sample_velocity_medium,
        Some("low") => instrument.sample_velocity_low,
        _ => instrument.sample_base_velocity,
    };
    (((u16::from(base) * u16::from(source_velocity.clamp(1, 127))) / 127).clamp(1, 127)) as u8
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

fn crossed_ppqn_boundary(previous: u64, current: u64, boundary: u64) -> bool {
    boundary > 0 && current >= boundary && previous / boundary != current / boundary
}

fn note_unit_to_pulses(unit: &str) -> u32 {
    match unit {
        "1/16" => 6,
        "1/8" => 12,
        "1/4" => 24,
        "1/2" => 48,
        "1/1" => 96,
        _ => DEFAULT_ALGORITHM_STEP_PULSES,
    }
}

fn note_unit_from_pulses(pulses: u32) -> &'static str {
    match pulses {
        6 => "1/16",
        12 => "1/8",
        24 => "1/4",
        48 => "1/2",
        96 => "1/1",
        _ => "1/8",
    }
}

fn clip_display_line(line: &str, width: usize) -> String {
    let mut out = String::new();
    for ch in line.chars().take(width) {
        out.push(ch);
    }
    out
}

fn scrolled_toast(toast: &NativeToast) -> String {
    const WIDTH: usize = 28;
    let chars = toast.message.chars().collect::<Vec<_>>();
    if chars.len() <= WIDTH {
        return toast.message.clone();
    }
    let span = chars.len() + 3;
    let offset = toast.offset % span;
    let mut padded = chars;
    padded.extend([' ', ' ', ' ']);
    padded.extend(toast.message.chars());
    padded.iter().skip(offset).take(WIDTH).collect()
}

fn dim_color(value: Value, divisor: i64) -> Value {
    let Some(object) = value.as_object() else {
        return value;
    };
    json!({
        "r": object.get("r").and_then(Value::as_i64).unwrap_or(0) / divisor,
        "g": object.get("g").and_then(Value::as_i64).unwrap_or(0) / divisor,
        "b": object.get("b").and_then(Value::as_i64).unwrap_or(0) / divisor,
    })
}

fn add_dim_white_overlay(value: &Value, amount: i64) -> Value {
    let Some(object) = value.as_object() else {
        return json!({ "r": amount, "g": amount, "b": amount });
    };
    json!({
        "r": (object.get("r").and_then(Value::as_i64).unwrap_or(0) + amount).min(255),
        "g": (object.get("g").and_then(Value::as_i64).unwrap_or(0) + amount).min(255),
        "b": (object.get("b").and_then(Value::as_i64).unwrap_or(0) + amount).min(255),
    })
}

fn scan_section_count(value: u8, size: usize) -> usize {
    match value {
        2 | 4 | 8 => usize::from(value).min(size),
        _ => 1,
    }
}

fn scan_index_for_overlay(tick: usize, span: usize, reverse: bool) -> usize {
    if span == 0 {
        return 0;
    }
    let index = tick % span;
    if reverse {
        span - 1 - index
    } else {
        index
    }
}

fn trigger_probability_allows(
    part: Option<&NativeSensePart>,
    map: &[String],
    rng: &mut u64,
    intent: &CellTriggerIntent,
) -> bool {
    let pct = trigger_probability_pct(part, map, intent.x, intent.y);
    if pct == 0 {
        return false;
    }
    if pct >= 100 {
        return true;
    }
    next_probability_random(rng) < f64::from(pct) / 100.0
}

fn trigger_probability_pct(
    part: Option<&NativeSensePart>,
    map: &[String],
    x: usize,
    y: usize,
) -> u8 {
    let Some(part) = part else {
        return 100;
    };
    match part.trigger_probability_mode.as_str() {
        "zero" => 0,
        "custom" => {
            let cell = map
                .get(y.saturating_mul(GRID_WIDTH).saturating_add(x))
                .map(String::as_str)
                .unwrap_or("full");
            match cell {
                "zero" => 0,
                "low" => part
                    .trigger_probability_low_pct
                    .min(part.trigger_probability_high_pct),
                "high" => part
                    .trigger_probability_high_pct
                    .max(part.trigger_probability_low_pct),
                _ => 100,
            }
        }
        _ => 100,
    }
}

fn next_probability_random(rng: &mut u64) -> f64 {
    *rng = rng
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*rng >> 11) as f64) / ((1_u64 << 53) as f64)
}

fn trigger_gate_color(mode: &str) -> Value {
    match mode {
        "zero" => json!({ "r": 220, "g": 0, "b": 0 }),
        "custom" => json!({ "r": 220, "g": 180, "b": 0 }),
        _ => json!({ "r": 0, "g": 220, "b": 0 }),
    }
}

fn touch_pan_pos_from_grid_x(x: usize) -> u8 {
    let cell = x.min(GRID_WIDTH - 1);
    let center_right = GRID_WIDTH / 2;
    let marker = if cell == center_right {
        center_right - 1
    } else if cell > center_right {
        cell - 1
    } else {
        cell
    };
    ((marker as f32 / (GRID_WIDTH - 2) as f32) * f32::from(PAN_POSITION_COUNT - 1)).round() as u8
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
            slot2_type: bus.slot2_type.clone(),
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
        "curve": "linear"
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
            | "sample.baseVelocity"
            | "midi.enabled"
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
    let action = if slot >= INSTRUMENT_SLOT_COUNT {
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
    if slot >= INSTRUMENT_SLOT_COUNT {
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

fn instrument_midi_enabled(instruments: &[NativeInstrumentSlot]) -> Vec<bool> {
    instruments
        .iter()
        .map(|instrument| instrument.midi_enabled)
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
            parse_slot_index(&value).map(|value| value.min(INSTRUMENT_SLOT_COUNT - 1))
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
    changed
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

fn apply_sense_payload(part: &mut NativeSensePart, payload: &Value) {
    assign_string(payload, "scanMode", &mut part.scan_mode);
    assign_string(payload, "scanAxis", &mut part.scan_axis);
    assign_string(payload, "scanUnit", &mut part.scan_unit);
    assign_string(payload, "scanDirection", &mut part.scan_direction);
    assign_u8(payload, "scanSections", &mut part.scan_sections, 8);
    if let Some(enabled) = payload.get("eventEnabled").and_then(Value::as_bool) {
        part.event_enabled = enabled;
    }
    if let Some(enabled) = payload.get("stateNotesEnabled").and_then(Value::as_bool) {
        part.state_notes_enabled = enabled;
    }
    assign_string(
        payload,
        "triggerProbabilityMode",
        &mut part.trigger_probability_mode,
    );
    assign_u8(
        payload,
        "triggerProbabilityLowPct",
        &mut part.trigger_probability_low_pct,
        100,
    );
    assign_u8(
        payload,
        "triggerProbabilityHighPct",
        &mut part.trigger_probability_high_pct,
        100,
    );
    if let Some(mapping) = payload.get("mapping") {
        assign_mapping(
            mapping,
            "scanned",
            &mut part.scanned_slot,
            &mut part.scanned_action,
        );
        assign_mapping(
            mapping,
            "scanned_empty",
            &mut part.scanned_empty_slot,
            &mut part.scanned_empty_action,
        );
        assign_mapping(
            mapping,
            "activate",
            &mut part.activate_slot,
            &mut part.activate_action,
        );
        assign_mapping(
            mapping,
            "stable",
            &mut part.stable_slot,
            &mut part.stable_action,
        );
        assign_mapping(
            mapping,
            "deactivate",
            &mut part.deactivate_slot,
            &mut part.deactivate_action,
        );
    }
    if let Some(pitch) = payload.get("pitch") {
        assign_u8(pitch, "lowestNote", &mut part.lowest_note, 127);
        assign_u8(pitch, "highestNote", &mut part.highest_note, 127);
        assign_u8(pitch, "startingNote", &mut part.starting_note, 127);
        assign_string(pitch, "scale", &mut part.scale);
        assign_string(pitch, "root", &mut part.root);
        assign_string(pitch, "outOfRange", &mut part.out_of_range);
    }
    if let Some(x) = payload.get("x") {
        assign_u8(x, "from", &mut part.x_from, 7);
        assign_u8(x, "to", &mut part.x_to, 7);
        if let Some(pitch) = x.get("pitch") {
            assign_bool(pitch, "enabled", &mut part.x_pitch_enabled);
            assign_i32(pitch, "steps", &mut part.x_pitch_steps, -16, 16);
            assign_bool(
                pitch,
                "restartEachSection",
                &mut part.x_pitch_restart_each_section,
            );
        }
        if let Some(lane) = x.get("velocity") {
            apply_value_lane_payload(&mut part.x_velocity, lane);
        }
        if let Some(lane) = x.get("filterCutoff") {
            apply_value_lane_payload(&mut part.x_filter_cutoff, lane);
        }
        if let Some(lane) = x.get("filterResonance") {
            apply_value_lane_payload(&mut part.x_filter_resonance, lane);
        }
    }
    if let Some(y) = payload.get("y") {
        assign_u8(y, "from", &mut part.y_from, 7);
        assign_u8(y, "to", &mut part.y_to, 7);
        if let Some(pitch) = y.get("pitch") {
            assign_bool(pitch, "enabled", &mut part.y_pitch_enabled);
            assign_i32(pitch, "steps", &mut part.y_pitch_steps, -16, 16);
            assign_bool(
                pitch,
                "restartEachSection",
                &mut part.y_pitch_restart_each_section,
            );
        }
        if let Some(lane) = y.get("velocity") {
            apply_value_lane_payload(&mut part.y_velocity, lane);
        }
        if let Some(lane) = y.get("filterCutoff") {
            apply_value_lane_payload(&mut part.y_filter_cutoff, lane);
        }
        if let Some(lane) = y.get("filterResonance") {
            apply_value_lane_payload(&mut part.y_filter_resonance, lane);
        }
    }
}

fn apply_value_lane_payload(target: &mut NativeValueLane, payload: &Value) {
    assign_bool(payload, "enabled", &mut target.enabled);
    assign_u8(payload, "from", &mut target.from, 127);
    assign_u8(payload, "to", &mut target.to, 127);
    assign_i32(payload, "gridOffset", &mut target.grid_offset, -7, 7);
}

fn assign_string(payload: &Value, key: &str, target: &mut String) {
    if let Some(value) = payload.get(key).and_then(Value::as_str) {
        *target = value.into();
    }
}

fn assign_u8(payload: &Value, key: &str, target: &mut u8, max: u8) {
    if let Some(value) = payload.get(key).and_then(Value::as_u64) {
        *target = (value as u8).min(max);
    }
}

fn assign_bool(payload: &Value, key: &str, target: &mut bool) {
    if let Some(value) = payload.get(key).and_then(Value::as_bool) {
        *target = value;
    }
}

fn assign_i32(payload: &Value, key: &str, target: &mut i32, min: i32, max: i32) {
    if let Some(value) = payload.get(key).and_then(Value::as_i64) {
        *target = (value as i32).clamp(min, max);
    }
}

fn assign_mapping(payload: &Value, key: &str, slot: &mut usize, action: &mut String) {
    let Some(mapping) = payload.get(key) else {
        return;
    };
    if let Some(value) = mapping.get("slot") {
        if value.as_str() == Some("none") {
            *slot = usize::MAX;
        } else if let Some(parsed) = value
            .as_str()
            .and_then(parse_slot_index)
            .or_else(|| value.as_u64().map(|value| value as usize))
        {
            *slot = parsed.min(INSTRUMENT_SLOT_COUNT - 1);
        }
    }
    if let Some(value) = mapping.get("action").and_then(Value::as_str) {
        *action = value.into();
    }
}

fn apply_aux_bindings_payload(bindings: &mut [Option<NativeAuxBinding>], payload: &Value) {
    for (index, binding) in bindings.iter_mut().enumerate() {
        let key = format!("aux{}", index + 1);
        let Some(value) = payload.get(&key) else {
            continue;
        };
        if value.is_null() {
            *binding = None;
            continue;
        }
        let turn_key = value
            .get("turnKey")
            .and_then(Value::as_str)
            .filter(|key| supported_aux_turn_key(key))
            .map(str::to_string);
        let press_action = value.get("pressAction").and_then(parse_aux_press_action);
        *binding = if turn_key.is_some() || press_action.is_some() {
            Some(NativeAuxBinding {
                turn_key,
                press_action,
            })
        } else {
            None
        };
    }
}

fn parse_aux_press_action(value: &Value) -> Option<NativeMenuAction> {
    match value.get("kind").and_then(Value::as_str)? {
        "behavior_action" => value
            .get("actionType")
            .and_then(Value::as_str)
            .map(|action| NativeMenuAction::BehaviorAction(action.into())),
        "platform_effect" => value
            .get("action")
            .and_then(Value::as_str)
            .map(|action| NativeMenuAction::PlatformEffect(action.into())),
        "reset_behavior" => Some(NativeMenuAction::ResetBehavior),
        _ => None,
    }
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

impl CoreRunner for NativeRunner {
    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        match message {
            HostMessage::TransportPulseStep {
                pulses,
                request_snapshot,
                ..
            } => {
                self.current_ppqn_pulse = self.current_ppqn_pulse.saturating_add(pulses as u64);
                let mut out = Vec::new();
                let events = self.advance_algorithm(pulses)?;
                if !events.is_empty() {
                    out.push(RunnerMessage::MusicalEvents { events });
                }
                if request_snapshot.unwrap_or(true) {
                    out.push(RunnerMessage::Snapshot {
                        snapshot: self.snapshot()?,
                    });
                }
                out.push(RunnerMessage::RuntimeStatus {
                    status: self.status(),
                });
                Ok(out)
            }
            HostMessage::DeviceInput { input } => {
                let input =
                    serde_json::from_value::<DeviceInput>(input).unwrap_or(DeviceInput::Other);
                self.handle_device_input(input)
            }
            HostMessage::MidiRealtimeStart => {
                if self.sync_source == SyncSource::External
                    && (!self.midi_clock_in_enabled || !self.midi_respond_to_start_stop)
                {
                    return self.messages_with_snapshot();
                }
                self.transport = RuntimeTransportState::Playing;
                self.current_ppqn_pulse = 0;
                self.algorithm_pulse_accumulator = 0;
                self.transport_flash = "measure";
                self.transport_flash_pulses_remaining = 6;
                self.messages_with_snapshot()
            }
            HostMessage::MidiRealtimeContinue => {
                if self.sync_source == SyncSource::External
                    && (!self.midi_clock_in_enabled || !self.midi_respond_to_start_stop)
                {
                    return self.messages_with_snapshot();
                }
                self.transport = RuntimeTransportState::Playing;
                self.messages_with_snapshot()
            }
            HostMessage::MidiRealtimeStop => {
                if self.sync_source == SyncSource::External
                    && (!self.midi_clock_in_enabled || !self.midi_respond_to_start_stop)
                {
                    return self.messages_with_snapshot();
                }
                self.transport = RuntimeTransportState::Stopped;
                self.transport_flash = "none";
                self.transport_flash_pulses_remaining = 0;
                self.event_dot_on = false;
                self.event_dot_pulses_remaining = 0;
                self.messages_with_snapshot()
            }
            HostMessage::MidiRealtimeClock { pulses } => {
                if self.sync_source == SyncSource::External && !self.midi_clock_in_enabled {
                    return self.messages_with_snapshot();
                }
                self.current_ppqn_pulse = self.current_ppqn_pulse.saturating_add(pulses as u64);
                if self.sync_source == SyncSource::External
                    && self.transport == RuntimeTransportState::Playing
                {
                    let events = self.advance_algorithm(pulses)?;
                    let mut out = Vec::new();
                    if !events.is_empty() {
                        out.push(RunnerMessage::MusicalEvents { events });
                    }
                    out.extend(self.messages_with_snapshot()?);
                    return Ok(out);
                }
                self.messages_with_snapshot()
            }
            HostMessage::RuntimeResult { result } => {
                self.apply_store_result(result)?;
                self.messages_with_snapshot()
            }
        }
    }
}

#[cfg(test)]
mod tests;
