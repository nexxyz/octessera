use crate::native_runner::play_pages::PlayMode;
use crate::native_runner::{
    AudioOptimization, AudioOutputSet, InstrumentType, ScanMode, TriggerProbabilityMode,
    UsbDataRole,
};
use crate::protocol::SyncSource;
use platform_core::{NoteBehavior, TriggerAction};
use realtime_engine::synth::{DspRuntimeConfig, VoiceStealingMode};

use super::{NativeMenuAction, NativeMenuItem, NativeParamBindingSpec};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeMenuConfig {
    pub behavior_id: String,
    pub behavior_ids: Vec<String>,
    pub build_items: Vec<NativeMenuItem>,
    pub build_items_by_layer: Vec<Vec<NativeMenuItem>>,
    pub random_seed: u16,
    pub behavior_target_items: Vec<Vec<NativeMenuItem>>,
    pub dsp_config: DspRuntimeConfig,
    pub layer_labels: Vec<String>,
    pub layer_names: Vec<String>,
    pub layer_auto_names: Vec<bool>,
    pub link_layers: Vec<NativeLinkLayerConfig>,
    pub active_layer_index: usize,
    pub link_lfos: [NativeLinkLfoConfig; 8],
    pub midi_cc_mappings: [NativeMidiCcConfig; 8],
    pub faders: NativeFadersConfig,
    pub param_mods: Vec<NativeParamModsConfig>,
    pub xy_x_binding: Option<NativeParamBindingSpec>,
    pub xy_y_binding: Option<NativeParamBindingSpec>,
    pub aux_auto_map_enabled: bool,
    pub aux_priority: String,
    pub aux_bindings: Vec<NativeAuxBindingConfig>,
    pub shift_aux_bindings: Vec<NativeAuxBindingConfig>,
    pub instrument_labels: Vec<String>,
    pub instrument_names: Vec<String>,
    pub instrument_types: Vec<InstrumentType>,
    pub instrument_auto_names: Vec<bool>,
    pub instrument_note_behaviors: Vec<NoteBehavior>,
    pub instrument_routes: Vec<String>,
    pub instrument_volumes: Vec<u8>,
    pub instrument_pan_positions: Vec<u8>,
    pub instrument_sample_slots: Vec<usize>,
    pub instrument_sample_paths: Vec<Vec<Option<String>>>,
    pub instrument_sample_availability: Vec<Vec<NativeSampleAvailability>>,
    pub instrument_synth_configs: Vec<serde_json::Value>,
    pub instrument_fm_configs: Vec<serde_json::Value>,
    pub instrument_pluck_configs: Vec<serde_json::Value>,
    pub instrument_drum_configs: Vec<serde_json::Value>,
    pub instrument_drum_selected_voices: Vec<usize>,
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
    pub instrument_sample_follow_transpose: Vec<bool>,
    pub instrument_sample_follow_pitch: Vec<bool>,
    pub instrument_tile_stamps: Vec<NativeMenuTileStamp>,
    /// Each instrument's selected sample slot's Start and Loop rows.
    pub instrument_sample_playback: Vec<NativeSamplePlayback>,
    pub instrument_midi_drums: Vec<NativeMenuMidiDrum>,
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
    pub voice_stealing_mode: VoiceStealingMode,
    pub auto_save_default: bool,
    pub rolling_backups: bool,
    pub ghost_tiles: bool,
    pub aux_auto_hints: bool,
    pub aux_manual_hints: bool,
    pub input_events_while_paused: bool,
    pub numeric_display_mode: String,
    pub main_font: String,
    pub small_font: String,
    pub home_visual: String,
    pub screen_sleep_seconds: u16,
    pub dim_timer_seconds: u16,
    pub grid_brightness: u8,
    pub display_brightness: u8,
    pub button_brightness: u8,
    pub midi_active: bool,
    pub midi_clock_out_enabled: bool,
    pub midi_clock_in_enabled: bool,
    pub midi_respond_to_start_stop: bool,
    pub midi_send_instruments: String,
    pub audio_outputs: AudioOutputSet,
    pub board: bool,
    pub audio_optimization: AudioOptimization,
    pub audio_optimization_capacity_available: bool,
    pub usb_data_role: UsbDataRole,
    pub usb_data_role_available: bool,
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
    pub bluetooth_enabled: bool,
    pub wifi_enabled: bool,
    pub bluetooth_audio: bool,
    pub bluetooth_paired: Vec<(String, String)>,
    pub bluetooth_found: Vec<(String, String)>,
    pub bluetooth_searching: bool,
    pub play_mode: PlayMode,
    pub play_fx_type: String,
    pub play_fx_target: String,
    pub play_fx_params: serde_json::Map<String, serde_json::Value>,
    pub xy_release: String,
    pub xy_smoothing_ms: u16,
    pub xy_invert_x: bool,
    pub xy_invert_y: bool,
    pub bpm: u16,
    pub swing_a: u8,
    pub swing_b: u8,
    pub sync_source: SyncSource,
}

/// The Bluetooth rows of System > Connections.
#[derive(Clone, Copy, Debug)]
pub struct NativeBluetoothRows<'a> {
    pub enabled: bool,
    pub audio: bool,
    pub paired: &'a [(String, String)],
    pub found: &'a [(String, String)],
    /// A Pair New scan is running; once it ends the page offers `Search again`.
    pub searching: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeSampleAvailability {
    Unknown,
    Available,
    Unavailable,
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
pub struct NativeLinkLayerConfig {
    pub scan_mode: ScanMode,
    pub scan_axis: String,
    pub scan_unit: String,
    pub scan_direction: String,
    pub scan_sections: u8,
    pub scanned_slot: usize,
    pub scanned_action: TriggerAction,
    pub scanned_empty_slot: usize,
    pub scanned_empty_action: TriggerAction,
    pub scanned_timing: LinkEventTimingConfig,
    pub scanned_empty_timing: LinkEventTimingConfig,
    pub event_enabled: bool,
    pub activate_slot: usize,
    pub activate_action: TriggerAction,
    pub activate_timing: LinkEventTimingConfig,
    pub stable_slot: usize,
    pub stable_action: TriggerAction,
    pub stable_timing: LinkEventTimingConfig,
    pub deactivate_slot: usize,
    pub deactivate_action: TriggerAction,
    pub deactivate_timing: LinkEventTimingConfig,
    pub trigger_probability_mode: TriggerProbabilityMode,
    pub trigger_probability_low_pct: u8,
    pub trigger_probability_high_pct: u8,
    pub state_notes_enabled: bool,
    pub seeded: bool,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeMidiCcConfig {
    pub cc: u8,
    /// 0 is Any.
    pub channel: u8,
    pub target: Option<NativeParamBindingSpec>,
}

/// The Play Faders page: each column's plain and Shift slider targets, the action columns and
/// the Tile Map rows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeFadersConfig {
    pub targets: [[Option<NativeParamBindingSpec>; 2]; 8],
    pub action_cols: u8,
    /// `none`, `action` or `param`.
    pub tile_type: String,
    pub tile_actions: [Option<NativeMenuAction>; 2],
    pub tile_target: Option<NativeParamBindingSpec>,
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

/// What Assign stamps onto grid tiles, as shown in the Sampler and Drum menus.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeMenuTileStamp {
    pub tune_semis: i8,
    pub tune_cents: i8,
    pub level_index: usize,
}

/// A sample slot's Start, Loop, Loop Start and Loop End, as the slot saves them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeSamplePlayback {
    pub loop_mode: &'static str,
    pub start_pct: u8,
    pub loop_start_pct: u8,
    pub loop_end_pct: u8,
}

pub const SAMPLE_LOOP_MODES: [&str; 3] = ["off", "forward", "pingpong"];
/// The factory Sampler kits: id, menu label, and the eight sample paths a load puts in the
/// slots.
pub const SAMPLER_KITS: [(&str, &str, [&str; 8]); 3] = [
    (
        "sdbkit",
        "SDB Kit",
        [
            "samples/Drum/kick/sdbkit-kick.wav",
            "samples/Drum/snare/sdbkit-snare.wav",
            "samples/Drum/hihat closed/sdbkit-hatclsd.wav",
            "samples/Drum/hihat open/sdbkit-hatopen.wav",
            "samples/Drum/toms/sdbkit-lotom.wav",
            "samples/Drum/toms/sdbkit-hitom.wav",
            "samples/Drum/claps/sdbkit-clap.wav",
            "samples/Drum/percussion/sdbkit-fmperc.wav",
        ],
    ),
    (
        "synthkit",
        "Synth Kit",
        [
            "samples/Drum/kick/synthkit-kick.wav",
            "samples/Drum/snare/synthkit-snare.wav",
            "samples/Drum/hihat closed/synthkit-hatclsd.wav",
            "samples/Drum/hihat open/synthkit-hatopen.wav",
            "samples/Drum/toms/synthkit-lotom.wav",
            "samples/Drum/toms/synthkit-hitom.wav",
            "samples/Drum/claps/synthkit-clap.wav",
            "samples/Drum/percussion/synthkit-8bit.wav",
        ],
    ),
    (
        "distkit",
        "Dist Kit",
        [
            "samples/Drum/kick/distkit-kick.wav",
            "samples/Drum/snare/distkit-snare.wav",
            "samples/Drum/hihat closed/distkit-hatclsd.wav",
            "samples/Drum/hihat open/distkit-hatopen.wav",
            "samples/Drum/toms/distkit-lotom.wav",
            "samples/Drum/toms/distkit-hitom.wav",
            "samples/Drum/claps/distkit-clap.wav",
            "samples/Drum/percussion/distkit-cowbell.wav",
        ],
    ),
];
/// Start and Loop Start stop one percent short of the sample's end, so a loop is never empty.
pub const SAMPLE_START_MAX_PCT: u8 = 99;

impl Default for NativeSamplePlayback {
    fn default() -> Self {
        Self {
            loop_mode: "off",
            start_pct: 0,
            loop_start_pct: 0,
            loop_end_pct: 100,
        }
    }
}

/// The MIDI Drum rows: output settings and the note and velocity Assign stamps next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeMenuMidiDrum {
    pub enabled: bool,
    pub channel: u8,
    pub duration_ms: u16,
    pub tile_note: u8,
    pub tile_velocity: u8,
}

/// The MIDI Drum ranges its rows, their edits, and loaded patches share.
pub const MIDI_DRUM_CHANNELS: (u16, u16) = (1, 16);
pub const MIDI_DRUM_DURATION_MS: (u16, u16) = (10, 2000);
pub const MIDI_DRUM_NOTES: (u16, u16) = (0, 127);
pub const MIDI_DRUM_VELOCITIES: (u16, u16) = (1, 127);

impl Default for NativeMenuMidiDrum {
    fn default() -> Self {
        Self {
            enabled: false,
            channel: 10,
            duration_ms: 100,
            tile_note: 36,
            tile_velocity: 100,
        }
    }
}

pub const TILE_STAMP_LEVELS: [&str; 4] = ["Base", "High", "Medium", "Low"];
