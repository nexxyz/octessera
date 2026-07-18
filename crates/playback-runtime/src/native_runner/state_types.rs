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
    pub(super) dim_timer_seconds: u16,
    pub(super) fn_held: bool,
    pub(super) shift_held: bool,
    pub(super) combined_modifier_held: bool,
    pub(super) fn_button_pressed: bool,
    pub(super) shift_button_pressed: bool,
    pub(super) combined_button_pressed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativePulsesLayer {
    pub(super) scan_mode: String,
    pub(super) scan_axis: String,
    pub(super) scan_unit: String,
    pub(super) scan_direction: String,
    pub(super) scan_sections: u8,
    pub(super) scanned_slot: usize,
    pub(super) scanned_action: String,
    pub(super) scanned_empty_slot: usize,
    pub(super) scanned_empty_action: String,
    pub(super) scanned_timing: LinkEventTiming,
    pub(super) scanned_empty_timing: LinkEventTiming,
    pub(super) event_enabled: bool,
    pub(super) activate_slot: usize,
    pub(super) activate_action: String,
    pub(super) activate_timing: LinkEventTiming,
    pub(super) stable_slot: usize,
    pub(super) stable_action: String,
    pub(super) stable_timing: LinkEventTiming,
    pub(super) deactivate_slot: usize,
    pub(super) deactivate_action: String,
    pub(super) deactivate_timing: LinkEventTiming,
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
    pub(super) arp: NativeLinkArp,
    pub(super) link_lfo: NativeLinkLfo,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeLinkArp {
    pub(super) mode: String,
    pub(super) source: String,
    pub(super) step_interval_steps: u8,
    pub(super) note_length_ms: u16,
    pub(super) gate_pct: u8,
    pub(super) octave_spread: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeLinkLfo {
    pub(super) enabled: bool,
    pub(super) target: Option<NativeParamBinding>,
    pub(super) period: String,
    pub(super) depth_pct: u8,
    pub(super) phase_pulses: u32,
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
    pub(super) slot3_type: String,
    pub(super) slot3_params: Value,
    pub(super) pan_pos: u8,
    pub(super) volume_pct: u8,
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
    pub(super) cancel_toast: Option<String>,
    pub(super) confirm_before_execute: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct LinkEventTiming {
    pub(super) delay_steps: u8,
    pub(super) retrigger_count: u8,
}

#[derive(Clone, Debug, Default)]
pub(super) struct DelayedRoutedEvents {
    pub(super) remaining_steps: u16,
    pub(super) events: RoutedMusicalEvents,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LinkArpHeldNote {
    pub(super) audio: bool,
    pub(super) channel: u8,
    pub(super) note: u8,
    pub(super) velocity: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeUsbSdTransferModal {
    pub(super) title: String,
    pub(super) lines: Vec<String>,
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
    pub(super) user_min: Option<f64>,
    pub(super) user_max: Option<f64>,
    pub(super) options: Vec<String>,
    pub(super) invert: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeParamMods {
    pub(super) x: Vec<Option<NativeParamBinding>>,
    pub(super) y: Vec<Option<NativeParamBinding>>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeSparksFxAssignment {
    pub(super) x: usize,
    pub(super) y: usize,
    pub(super) config: Value,
}

impl Default for NativePulsesLayer {
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
            scanned_timing: LinkEventTiming::default(),
            scanned_empty_timing: LinkEventTiming::default(),
            event_enabled: true,
            activate_slot: 0,
            activate_action: "note_on".into(),
            activate_timing: LinkEventTiming::default(),
            stable_slot: 0,
            stable_action: "none".into(),
            stable_timing: LinkEventTiming::default(),
            deactivate_slot: 0,
            deactivate_action: "note_off".into(),
            deactivate_timing: LinkEventTiming::default(),
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
            arp: NativeLinkArp::default(),
            link_lfo: NativeLinkLfo::default(),
        }
    }
}

impl Default for NativeLinkArp {
    fn default() -> Self {
        Self {
            mode: "none".into(),
            source: "simultaneous".into(),
            step_interval_steps: 1,
            note_length_ms: 120,
            gate_pct: 80,
            octave_spread: 0,
        }
    }
}

impl Default for NativeLinkLfo {
    fn default() -> Self {
        Self {
            enabled: false,
            target: None,
            period: "1/1".into(),
            depth_pct: 100,
            phase_pulses: 0,
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
            name: "None".into(),
            slot1_type: "none".into(),
            slot1_params: json!({}),
            slot2_type: "none".into(),
            slot2_params: json!({}),
            slot3_type: "none".into(),
            slot3_params: json!({}),
            pan_pos: 16,
            volume_pct: 100,
            auto_name: true,
        }
    }
}

impl Default for NativeUiState {
    fn default() -> Self {
        Self {
            display_brightness: 75,
            grid_brightness: 25,
            button_brightness: 35,
            master_volume: 73,
            ghost_cells: false,
            numeric_display_mode: "bar+numbers".into(),
            screen_sleep_seconds: 60,
            dim_timer_seconds: 60,
            fn_held: false,
            shift_held: false,
            combined_modifier_held: false,
            fn_button_pressed: false,
            shift_button_pressed: false,
            combined_button_pressed: false,
        }
    }
}
