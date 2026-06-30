use super::*;

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
        slot.name = derive_instrument_name(index, "none");
        slot.auto_name = true;
        slot.midi_enabled = false;
        slot.midi_channel = (index + 1).min(16) as u8;
        slot
    }
}
