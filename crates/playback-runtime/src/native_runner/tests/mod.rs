use super::*;

pub(super) fn pi_audio_command_rejection(command: &RuntimeAudioCommand) -> Option<String> {
    use realtime_engine::synth::{
        validate_fm_param_path, validate_pluck_param_path, validate_sample_bank_param_path,
        validate_synth_param_path, DrumParamId,
    };
    match command {
        RuntimeAudioCommand::SetSynthParam { path, .. } => validate_synth_param_path(path).err(),
        RuntimeAudioCommand::SetFmParam { path, .. } => validate_fm_param_path(path).err(),
        RuntimeAudioCommand::SetPluckParam { path, .. } => validate_pluck_param_path(path).err(),
        RuntimeAudioCommand::SetSampleBankParam { path, .. } => {
            validate_sample_bank_param_path(path).err()
        }
        RuntimeAudioCommand::SetDrumParam { path, .. } => DrumParamId::from_path(path)
            .is_none()
            .then(|| format!("unsupported drum parameter path `{path}`")),
        _ => None,
    }
}

impl NativeRunner {
    pub(super) fn apply_current_menu_edit(&mut self) -> Result<(), String> {
        let key = self
            .menu
            .current_key()
            .map(str::to_string)
            .ok_or("no menu row is selected")?;
        self.apply_or_schedule_menu_key(&key)?;
        if let Some(pending) = self.pending.pending_menu_apply.as_mut() {
            pending.due_at = std::time::Instant::now();
            self.apply_due_menu_key(std::time::Instant::now())?;
        }
        Ok(())
    }

    pub(super) fn edit_menu_key(&mut self, key: &str, delta: i8) -> Result<(), String> {
        assert!(self.menu.focus_item_key(key), "{key} is not in the menu");
        self.menu.turn_key(key, delta);
        self.apply_current_menu_edit()
    }

    pub(super) fn apply_menu_key_edit(&mut self, key: &str) -> Result<(), String> {
        assert!(self.menu.focus_item_key(key), "{key} is not in the menu");
        self.apply_current_menu_edit()
    }
}

mod audio_menu;
mod audio_menu_direct;
mod audio_menu_naming;
mod audio_optimization;
mod audio_outputs_config;
mod audio_outputs_menu;
mod audio_restart;
mod audio_restart_follow_up;
mod audio_restart_persistence;
mod audio_restart_persistence_usb;
mod aux_auto_map;
mod basics;
mod behavior_menu_defaults;
mod behavior_palette;
mod bluetooth;
mod browser_and_help;
mod canonical_defaults;
mod config_dto;
mod config_field_partition;
mod config_persistence;
mod config_schema_validation_matrix;
mod config_snapshot;
mod config_transactions;
mod construction_defaults;
mod controls;
mod dim_sleep;
mod display_transients;
mod drum_commands;
mod drum_input;
mod drum_model;
mod drum_routing;
mod duck_fx_ranges;
mod external_resync_boundary;
mod factory_instrument_load;
mod factory_kit_assets;
mod fast_dispatch_parity;
mod fm;
mod fm_device;
mod happy_path;
mod hdmi;
mod input_events;
mod input_events_midi;
mod instrument_binding_targets;
mod instrument_fast_bindings;
mod instruments;
mod layer_replacement;
mod layer_trigger_gate_device_replay;
mod layer_trigger_gate_release;
mod life_mapping;
mod link_and_shape_menu;
mod looper;
mod menu_edit_dispatch;
mod menu_navigation;
mod menu_navigation_state;
mod modulation;
mod modulation_behavior_targets;
mod modulation_bindings;
mod modulation_layer_replacement;
mod modulation_lfo_phase;
mod modulation_runtime;
mod modulation_runtime_commands;
mod modulation_runtime_fx;
mod note_set_runtime;
mod note_sets;
mod numeric_binding_round_trip;
mod play_fx;
mod play_menu;
mod play_overlay;
mod pluck;
mod pluck_audio;
mod pluck_device;
mod portable_patch;
mod portable_patch_samples;
mod recording;
mod restart_dialog_snapshots;
mod runtime_control;
mod runtime_transport;
mod sample_browser_store;
mod scan_replacement;
mod selected_menu_row;
mod sequencer_transport_origin;
mod setup_portal;
mod show_hold;
mod shutdown;
mod snapshot_autosave;
mod snapshot_runtime;
mod step_rates;
mod store;
mod store_result_contracts;
mod structural_draft;
mod transport_origins;
mod transport_phase_resets;
mod trigger_gates;
mod twinkle;
mod ui_scenario;
mod user_data_restore;
mod user_data_restore_rehydration;
mod user_data_transfer;
mod xy_smoothing;

pub(crate) fn snapshot_from(messages: &[RunnerMessage]) -> Value {
    messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::Snapshot { snapshot } => Some(snapshot.clone()),
            _ => None,
        })
        .expect("snapshot message")
}

pub(crate) fn led_cells(snapshot: &Value) -> Vec<Value> {
    let rgb = snapshot["leds"]["rgb"].as_array().expect("led rgb array");
    (0..64)
        .map(|index| {
            let offset = index * 3;
            json!({
                "r": rgb[offset].as_u64().unwrap(),
                "g": rgb[offset + 1].as_u64().unwrap(),
                "b": rgb[offset + 2].as_u64().unwrap(),
            })
        })
        .collect()
}

pub(crate) fn led_rgb(rgb: [u8; 3]) -> Value {
    json!({ "r": rgb[0], "g": rgb[1], "b": rgb[2] })
}

pub(crate) fn dim_rgb(rgb: [u8; 3], divisor: u8) -> [u8; 3] {
    let divisor = divisor.max(1);
    [rgb[0] / divisor, rgb[1] / divisor, rgb[2] / divisor]
}

pub(crate) fn assert_rejected_without_byte_changes(runner: &mut NativeRunner, payload: Value) {
    let before_config = serde_json::to_vec(&runner.config_payload()).unwrap();
    let before_snapshot = serde_json::to_vec(&runner.snapshot().unwrap()).unwrap();

    assert!(runner.apply_config_payload(payload).is_err());

    assert_eq!(
        serde_json::to_vec(&runner.config_payload()).unwrap(),
        before_config
    );
    assert_eq!(
        serde_json::to_vec(&runner.snapshot().unwrap()).unwrap(),
        before_snapshot
    );
}

pub(crate) fn unversioned_payload(mut payload: Value) -> Value {
    if let Some(object) = payload.as_object_mut() {
        object.remove("kind");
        object.remove("schemaVersion");
        object.remove("revision");
    }
    payload
}

pub(crate) fn confirm_current_dialog(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner.display.confirm_dialog.as_mut().unwrap().cursor = 1;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap()
}

pub(crate) fn select_behavior(runner: &mut NativeRunner, behavior_id: &str) {
    runner
        .execute_menu_action(crate::native_menu::NativeMenuAction::SelectBehavior(
            behavior_id.into(),
        ))
        .unwrap();
}

pub(crate) fn musical_note_ons(messages: &[RunnerMessage]) -> Vec<(u8, u8)> {
    messages
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::MusicalEvents { events } => events.as_slice(),
            _ => &[],
        })
        .filter_map(|event| match event {
            platform_core::MusicalEvent::NoteOn { channel, note, .. } => Some((*channel, *note)),
            _ => None,
        })
        .collect()
}
