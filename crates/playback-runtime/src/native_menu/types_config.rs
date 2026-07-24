use crate::protocol::SyncSource;

use super::{NativeMenuAction, NativeMenuItem, NativeParamBindingSpec};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeMenuConfig {
    pub behavior_id: String,
    pub behavior_ids: Vec<String>,
    pub worlds_items: Vec<NativeMenuItem>,
    pub worlds_items_by_layer: Vec<Vec<NativeMenuItem>>,
    pub behavior_target_items: Vec<Vec<NativeMenuItem>>,
    pub layer_labels: Vec<String>,
    pub layer_names: Vec<String>,
    pub layer_auto_names: Vec<bool>,
    pub pulses_layers: Vec<NativePulsesLayerConfig>,
    pub active_layer_index: usize,
    pub link_lfos: [NativeLinkLfoConfig; 8],
    pub param_mods: Vec<NativeParamModsConfig>,
    pub xy_x_binding: Option<NativeParamBindingSpec>,
    pub xy_y_binding: Option<NativeParamBindingSpec>,
    pub aux_auto_map_enabled: bool,
    pub aux_bindings: Vec<NativeAuxBindingConfig>,
    pub shift_aux_bindings: Vec<NativeAuxBindingConfig>,
    pub instrument_labels: Vec<String>,
    pub instrument_names: Vec<String>,
    pub instrument_types: Vec<String>,
    pub instrument_auto_names: Vec<bool>,
    pub instrument_note_behaviors: Vec<String>,
    pub instrument_routes: Vec<String>,
    pub instrument_volumes: Vec<u8>,
    pub instrument_pan_positions: Vec<u8>,
    pub instrument_sample_slots: Vec<usize>,
    pub instrument_sample_paths: Vec<Vec<Option<String>>>,
    pub instrument_synth_configs: Vec<serde_json::Value>,
    pub instrument_synth_osc1_waveforms: Vec<String>,
    pub instrument_synth_osc2_waveforms: Vec<String>,
    pub instrument_synth_filter_types: Vec<String>,
    pub instrument_synth_filter_cutoffs: Vec<u16>,
    pub instrument_synth_gain_pct: Vec<u8>,
    pub instrument_synth_filter_resonance: Vec<u8>,
    pub instrument_sample_tune_semis: Vec<i8>,
    pub instrument_sample_gain_pct: Vec<u8>,
    pub instrument_sample_base_velocity: Vec<u8>,
    pub instrument_sample_amp_velocity_sensitivity_pct: Vec<u8>,
    pub instrument_sample_velocity_levels_enabled: Vec<bool>,
    pub instrument_sample_velocity_high: Vec<u8>,
    pub instrument_sample_velocity_medium: Vec<u8>,
    pub instrument_sample_velocity_low: Vec<u8>,
    pub instrument_sample_amp_envs: Vec<serde_json::Value>,
    pub instrument_sample_filters: Vec<serde_json::Value>,
    pub instrument_sample_filter_envs: Vec<serde_json::Value>,
    pub instrument_midi_enabled: Vec<bool>,
    pub instrument_midi_channels: Vec<u8>,
    pub instrument_midi_velocity: Vec<u8>,
    pub instrument_midi_duration_ms: Vec<u16>,
    pub fx_buses: Vec<NativeFxBusConfig>,
    pub global_fx_slots: Vec<String>,
    pub global_fx_params: Vec<serde_json::Value>,
    pub sample_browser: Option<NativeSampleBrowserConfig>,
    pub sample_favourite_dirs: Vec<String>,
    pub sample_builtin_favourite_dirs: Vec<String>,
    pub algorithm_step_pulses: u32,
    pub master_volume: u8,
    pub note_length_ms: u16,
    pub velocity_scale_pct: u16,
    pub velocity_curve: String,
    pub voice_stealing_mode: String,
    pub auto_save_default: bool,
    pub rolling_backups: bool,
    pub ghost_cells: bool,
    pub input_events_while_paused: bool,
    pub numeric_display_mode: String,
    pub screen_sleep_seconds: u16,
    pub dim_timer_seconds: u16,
    pub grid_brightness: u8,
    pub display_brightness: u8,
    pub button_brightness: u8,
    pub midi_enabled: bool,
    pub midi_clock_out_enabled: bool,
    pub midi_clock_in_enabled: bool,
    pub midi_respond_to_start_stop: bool,
    pub usb_audio_out: String,
    pub usb_midi_out_enabled: bool,
    pub recording_max_minutes: u16,
    pub hdmi_mode: String,
    pub hdmi_show_gridlines: bool,
    pub hdmi_cycle_measures: u8,
    pub preset_names: Vec<String>,
    pub preset_draft_name: String,
    pub preset_rename_source: Option<String>,
    pub midi_outputs: Vec<(String, String)>,
    pub midi_inputs: Vec<(String, String)>,
    pub sparks_mode: String,
    pub sparks_fx_type: String,
    pub sparks_fx_target: String,
    pub sparks_fx_params: serde_json::Map<String, serde_json::Value>,
    pub xy_release: String,
    pub xy_invert_x: bool,
    pub xy_invert_y: bool,
    pub bpm: u16,
    pub swing_pct: u8,
    pub audio_output_buffer_frames: u32,
    pub sync_source: SyncSource,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct NativeParamModsConfig {
    pub x: [Option<NativeParamBindingSpec>; 2],
    pub y: [Option<NativeParamBindingSpec>; 2],
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct NativeAuxBindingConfig {
    pub turn: Option<NativeParamBindingSpec>,
    pub click: Option<NativeMenuAction>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativePulsesLayerConfig {
    pub scan_mode: String,
    pub scan_axis: String,
    pub scan_unit: String,
    pub scan_direction: String,
    pub scan_sections: u8,
    pub scanned_slot: usize,
    pub scanned_action: String,
    pub scanned_empty_slot: usize,
    pub scanned_empty_action: String,
    pub scanned_timing: LinkEventTimingConfig,
    pub scanned_empty_timing: LinkEventTimingConfig,
    pub event_enabled: bool,
    pub activate_slot: usize,
    pub activate_action: String,
    pub activate_timing: LinkEventTimingConfig,
    pub stable_slot: usize,
    pub stable_action: String,
    pub stable_timing: LinkEventTimingConfig,
    pub deactivate_slot: usize,
    pub deactivate_action: String,
    pub deactivate_timing: LinkEventTimingConfig,
    pub trigger_probability_mode: String,
    pub trigger_probability_low_pct: u8,
    pub trigger_probability_high_pct: u8,
    pub state_notes_enabled: bool,
    pub lowest_note: u8,
    pub highest_note: u8,
    pub starting_note: u8,
    pub scale: String,
    pub root: String,
    pub out_of_range: String,
    pub x_pitch_enabled: bool,
    pub x_pitch_steps: i32,
    pub x_pitch_restart_each_section: bool,
    pub y_pitch_enabled: bool,
    pub y_pitch_steps: i32,
    pub y_pitch_restart_each_section: bool,
    pub x_from: u8,
    pub x_to: u8,
    pub x_velocity: NativeValueLaneConfig,
    pub x_filter_cutoff: NativeValueLaneConfig,
    pub x_filter_resonance: NativeValueLaneConfig,
    pub y_from: u8,
    pub y_to: u8,
    pub y_velocity: NativeValueLaneConfig,
    pub y_filter_cutoff: NativeValueLaneConfig,
    pub y_filter_resonance: NativeValueLaneConfig,
    pub arp: NativeLinkArpConfig,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeLinkArpConfig {
    pub mode: String,
    pub source: String,
    pub step_interval_steps: u8,
    pub note_length_ms: u16,
    pub gate_pct: u8,
    pub octave_spread: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeLinkLfoConfig {
    pub enabled: bool,
    pub target: Option<NativeParamBindingSpec>,
    pub period: String,
    pub depth_pct: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LinkEventTimingConfig {
    pub delay_steps: u8,
    pub retrigger_count: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeValueLaneConfig {
    pub enabled: bool,
    pub from: u8,
    pub to: u8,
    pub grid_offset: i32,
    pub curve: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeSampleBrowserConfig {
    pub instrument_slot: usize,
    pub sample_slot: usize,
    pub dir: String,
    pub entries: Vec<NativeSampleEntryConfig>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeSampleEntryConfig {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeFxBusConfig {
    pub name: String,
    pub slot1_type: String,
    pub slot1_params: serde_json::Value,
    pub slot2_type: String,
    pub slot2_params: serde_json::Value,
    pub slot3_type: String,
    pub slot3_params: serde_json::Value,
    pub pan_pos: u8,
    pub volume_pct: u8,
    pub auto_name: bool,
}
