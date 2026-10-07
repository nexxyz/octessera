use super::*;
use std::sync::Arc;

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

#[derive(Clone, Default)]
pub(super) struct NativePendingState {
    pub(super) pending_save_revision: Option<u64>,
    pub(super) pending_default_load_request: Option<(String, Option<u64>)>,
    pub(super) saved_patch_baseline: Option<Arc<Value>>,
    pub(super) system_persistence: super::system_persistence::SystemPersistenceState,
    pub(super) pending_autosave_payload_due_at: Option<Instant>,
    pub(super) autosave_payload_notified_at: Option<Instant>,
    pub(super) manual_save_request: Option<NativeManualSaveRequest>,
    pub(super) native_preset_write: Option<NativePendingPresetWrite>,
    pub(super) pending_aux_turn_toast: Option<PendingNativeToast>,
    pub(super) pending_menu_apply: Option<PendingMenuApply>,
    pub(super) suppress_snapshot_response: bool,
    pub(super) presentation_deferred: bool,
    pub(super) external_autosave_deferred: bool,
    pub(super) drum_hits: Vec<crate::protocol::DrumHit>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativePendingPresetWrite {
    pub(super) request_id: String,
    pub(super) revision: u64,
    pub(super) request: NativeManualSaveRequest,
    pub(super) catalog: Option<Vec<String>>,
    pub(super) cleanup_error: Option<String>,
}

#[derive(Clone)]
pub(super) struct NativeDisplayState {
    pub(super) ui: NativeUiState,
    pub(super) hdmi: NativeHdmiConfig,
    pub(super) oled_mode: NativeOledMode,
    pub(super) oled_splash_text: String,
    pub(super) oled_splash_until: Option<Instant>,
    pub(super) startup_splash_presented: bool,
    pub(super) last_interaction_at: Instant,
    pub(super) aux_mapping_peek_visible: bool,
    pub(super) modifier_hint_started_at: Option<Instant>,
    pub(super) help_popup: Option<NativeHelpPopup>,
    pub(super) confirm_dialog: Option<NativeConfirmDialog>,
    pub(super) usb_sd_transfer_modal: Option<NativeUsbSdTransferModal>,
    pub(super) system_info_modal: Option<NativeSystemInfoModal>,
    pub(super) setup_portal: Option<NativeSetupPortalState>,
    pub(super) user_data_transfer: Option<NativeUserDataTransferState>,
    pub(super) user_data_restore: Option<NativeUserDataRestoreState>,
    pub(super) transients: display_transients::DisplayTransients,
    pub(super) auto_save_flash_serial: u64,
    pub(super) auto_save_flash_until: Option<Instant>,
    pub(super) toast: Option<NativeToast>,
    pub(super) toast_expires_at: Option<Instant>,
    pub(super) aux_turn_toast_cooldown_until: Option<Instant>,
    pub(super) held_toast_head: Option<String>,
    pub(super) menu_scroll_offset: usize,
    pub(super) runtime_error_presentation: Option<NativeRuntimeErrorPresentation>,
}

impl NativeDisplayState {
    pub(super) fn new(ui: NativeUiState, now: Instant, hdmi: NativeHdmiConfig) -> Self {
        Self {
            ui,
            hdmi,
            oled_mode: NativeOledMode::Splash,
            oled_splash_text: OLED_STARTUP_SPLASH_KEY.into(),
            oled_splash_until: Some(now + Duration::from_millis(OLED_STARTUP_SPLASH_MS)),
            startup_splash_presented: false,
            last_interaction_at: now,
            aux_mapping_peek_visible: false,
            modifier_hint_started_at: None,
            help_popup: None,
            confirm_dialog: None,
            usb_sd_transfer_modal: None,
            system_info_modal: None,
            setup_portal: None,
            user_data_transfer: None,
            user_data_restore: None,
            transients: display_transients::DisplayTransients::new(now),
            auto_save_flash_serial: 0,
            auto_save_flash_until: None,
            toast: None,
            toast_expires_at: None,
            aux_turn_toast_cooldown_until: None,
            held_toast_head: None,
            menu_scroll_offset: 0,
            runtime_error_presentation: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeSetupPortalState {
    pub(super) status: RuntimeSetupPortalStatus,
    pub(super) request_id: Option<String>,
    pub(super) revision: Option<u64>,
    pub(super) visible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeUserDataTransferState {
    pub(super) status: RuntimeUserDataTransferStatus,
    pub(super) request_id: Option<String>,
    pub(super) revision: Option<u64>,
    pub(super) visible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeRuntimeErrorPresentation {
    pub(super) title: String,
    pub(super) lines: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeTransportState {
    pub(super) transport: RuntimeTransportState,
    pub(super) sync_source: SyncSource,
    pub(super) pending_resync: bool,
    pub(super) bpm: f64,
    pub(super) swing_pct: u8,
    pub(super) current_ppqn_pulse: u64,
    pub(super) swung_ppqn_pulse: u64,
    pub(super) tick: u64,
    pub(super) layer_ticks: Vec<u64>,
    pub(super) algorithm_step_pulses: u32,
    pub(super) algorithm_pulse_accumulator: u32,
    pub(super) layer_algorithm_step_pulses: Vec<u32>,
    pub(super) layer_pulse_accumulators: Vec<u32>,
}

impl NativeTransportState {
    pub(super) fn new(
        bpm: f64,
        swing_pct: u8,
        sync_source: SyncSource,
        algorithm_step_pulses: u32,
    ) -> Self {
        Self {
            transport: RuntimeTransportState::Stopped,
            sync_source,
            pending_resync: false,
            bpm,
            swing_pct,
            current_ppqn_pulse: 0,
            swung_ppqn_pulse: 0,
            tick: 0,
            layer_ticks: vec![0; LAYER_COUNT],
            algorithm_step_pulses,
            algorithm_pulse_accumulator: 0,
            layer_algorithm_step_pulses: vec![algorithm_step_pulses; LAYER_COUNT],
            layer_pulse_accumulators: vec![0; LAYER_COUNT],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeHdmiConfig {
    pub(super) mode: String,
    pub(super) show_gridlines: bool,
    pub(super) cycle_measures: u8,
    pub(super) source_layer_index: usize,
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
    pub(super) head: String,
    pub(super) value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeXyTouch {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) display_x: f32,
    pub(super) display_y: f32,
    pub(super) active: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeXyGlide {
    pub(super) from: f32,
    pub(super) target: f32,
    pub(super) started_at: Instant,
    pub(super) duration: Duration,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativePlayFxAssignment {
    pub(super) x: usize,
    pub(super) y: usize,
    pub(super) config: Value,
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
