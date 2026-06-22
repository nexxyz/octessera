use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum NativeOledMode {
    Normal,
    Splash,
    Off,
}

#[derive(Clone, Debug)]
pub(super) struct NativeUiState {
    pub(super) display_brightness: u8,
    pub(super) grid_brightness: u8,
    pub(super) button_brightness: u8,
    pub(super) master_volume: u8,
    pub(super) ghost_cells: bool,
    pub(super) numeric_display_mode: String,
    pub(super) screen_sleep_seconds: u16,
    pub(super) fn_held: bool,
    pub(super) shift_held: bool,
    pub(super) combined_modifier_held: bool,
    pub(super) fn_button_pressed: bool,
    pub(super) shift_button_pressed: bool,
    pub(super) combined_button_pressed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeInstrumentSlot {
    pub(super) kind: String,
    pub(super) note_behavior: String,
    pub(super) auto_name: bool,
    pub(super) name: String,
    pub(super) volume: u8,
    pub(super) pan_pos: u8,
    pub(super) route: String,
    pub(super) selected_sample_slot: usize,
    pub(super) sample_paths: Vec<Option<String>>,
    pub(super) sample_assignments: Vec<NativeSampleAssignment>,
    pub(super) synth_config: Value,
    pub(super) synth_gain_pct: u8,
    pub(super) sample_tune_semis: i8,
    pub(super) sample_gain_pct: u8,
    pub(super) sample_amp_env: Value,
    pub(super) sample_filter: Value,
    pub(super) sample_filter_env: Value,
    pub(super) sample_base_velocity: u8,
    pub(super) sample_amp_velocity_sensitivity_pct: u8,
    pub(super) sample_velocity_levels_enabled: bool,
    pub(super) sample_velocity_high: u8,
    pub(super) sample_velocity_medium: u8,
    pub(super) sample_velocity_low: u8,
    pub(super) midi_enabled: bool,
    pub(super) midi_channel: u8,
    pub(super) midi_velocity: u8,
    pub(super) midi_duration_ms: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeSampleAssignment {
    pub(super) x: usize,
    pub(super) y: usize,
    pub(super) sample_slot: usize,
    pub(super) level: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeSampleBrowser {
    pub(super) instrument_slot: usize,
    pub(super) sample_slot: usize,
    pub(super) dir: String,
    pub(super) entries: Vec<SampleEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeSensePart {
    pub(super) scan_mode: String,
    pub(super) scan_axis: String,
    pub(super) scan_unit: String,
    pub(super) scan_direction: String,
    pub(super) scan_sections: u8,
    pub(super) scanned_slot: usize,
    pub(super) scanned_action: String,
    pub(super) scanned_empty_slot: usize,
    pub(super) scanned_empty_action: String,
    pub(super) event_enabled: bool,
    pub(super) activate_slot: usize,
    pub(super) activate_action: String,
    pub(super) stable_slot: usize,
    pub(super) stable_action: String,
    pub(super) deactivate_slot: usize,
    pub(super) deactivate_action: String,
    pub(super) trigger_probability_mode: String,
    pub(super) trigger_probability_low_pct: u8,
    pub(super) trigger_probability_high_pct: u8,
    pub(super) state_notes_enabled: bool,
    pub(super) lowest_note: u8,
    pub(super) highest_note: u8,
    pub(super) starting_note: u8,
    pub(super) scale: String,
    pub(super) root: String,
    pub(super) out_of_range: String,
    pub(super) x_pitch_enabled: bool,
    pub(super) x_pitch_steps: i32,
    pub(super) x_pitch_restart_each_section: bool,
    pub(super) y_pitch_enabled: bool,
    pub(super) y_pitch_steps: i32,
    pub(super) y_pitch_restart_each_section: bool,
    pub(super) x_from: u8,
    pub(super) x_to: u8,
    pub(super) x_velocity: NativeValueLane,
    pub(super) x_filter_cutoff: NativeValueLane,
    pub(super) x_filter_resonance: NativeValueLane,
    pub(super) y_from: u8,
    pub(super) y_to: u8,
    pub(super) y_velocity: NativeValueLane,
    pub(super) y_filter_cutoff: NativeValueLane,
    pub(super) y_filter_resonance: NativeValueLane,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeAuxBinding {
    pub(super) turn_key: Option<String>,
    pub(super) press_action: Option<NativeMenuAction>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeFxBus {
    pub(super) name: String,
    pub(super) slot1_type: String,
    pub(super) slot1_params: Value,
    pub(super) slot2_type: String,
    pub(super) slot2_params: Value,
    pub(super) pan_pos: u8,
    pub(super) auto_name: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeHelpPopup {
    pub(super) title: String,
    pub(super) lines: Vec<String>,
    pub(super) scroll: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeConfirmDialog {
    pub(super) title: String,
    pub(super) lines: Vec<String>,
    pub(super) options: Vec<String>,
    pub(super) cursor: usize,
    pub(super) action: NativeMenuAction,
}

#[derive(Clone, Debug)]
pub(super) struct NativeToast {
    pub(super) message: String,
    pub(super) offset: usize,
}

#[derive(Clone, Debug)]
pub(super) struct PendingNativeToast {
    pub(super) message: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeXyTouch {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) display_x: f32,
    pub(super) display_y: f32,
    pub(super) active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeValueLane {
    pub(super) enabled: bool,
    pub(super) from: u8,
    pub(super) to: u8,
    pub(super) grid_offset: i32,
    pub(super) curve: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeParamBinding {
    pub(super) key: String,
    pub(super) label: Option<String>,
    pub(super) kind: String,
    pub(super) min: Option<f64>,
    pub(super) max: Option<f64>,
    pub(super) step: Option<f64>,
    pub(super) options: Vec<String>,
    pub(super) invert: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeParamMods {
    pub(super) x: Vec<Option<NativeParamBinding>>,
    pub(super) y: Vec<Option<NativeParamBinding>>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeDanceFxAssignment {
    pub(super) x: usize,
    pub(super) y: usize,
    pub(super) config: Value,
}

impl NativeInstrumentSlot {
    pub(super) fn new(index: usize) -> Self {
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

    pub(super) fn reset(index: usize) -> Self {
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
            scan_mode: "none".into(),
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
    pub(super) fn velocity_default() -> Self {
        Self {
            enabled: false,
            from: 1,
            to: 127,
            grid_offset: 0,
            curve: "linear".into(),
        }
    }

    pub(super) fn filter_cutoff_default() -> Self {
        Self {
            enabled: false,
            from: 20,
            to: 127,
            grid_offset: 0,
            curve: "linear".into(),
        }
    }

    pub(super) fn filter_resonance_default() -> Self {
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
            fn_button_pressed: false,
            shift_button_pressed: false,
            combined_button_pressed: false,
        }
    }
}
