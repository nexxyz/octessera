use super::super::grid_assign::GridAssign;
use super::super::midi_cc_in::MIDI_CC_SLOT_COUNT;
use super::super::play_pages::{ActivePlayMode, PlayMode};
use super::super::{NativeRunner, RuntimeTransportState, GRID_HEIGHT};
use crate::native_menu::PlatformAction;
use crate::protocol::RuntimePlatformEffect;

impl NativeRunner {
    /// Runs a platform action: runner-side ones change the runner, the rest become the host
    /// effect `platform_effect_for_action` builds.
    pub(in crate::native_runner) fn execute_platform_action(
        &mut self,
        action: PlatformAction,
    ) -> Result<Option<RuntimePlatformEffect>, String> {
        match action {
            PlatformAction::RestartSaveChoice | PlatformAction::RestartSaving => Ok(None),
            PlatformAction::RestartApp => self.restart_app_after_save(),
            PlatformAction::PresetRenamePick(name) => {
                self.patch_store.preset_draft_name = name.clone();
                self.patch_store.preset_rename_source = Some(name);
                Ok(None)
            }
            PlatformAction::PresetSaveCurrent if self.patch_store.current_preset_name.is_none() => {
                self.show_toast("No preset loaded");
                Ok(None)
            }
            PlatformAction::PresetSaveAs
            | PlatformAction::PresetRenameApply
            | PlatformAction::PresetSaveCurrent => match self.platform_effect_for_action(&action) {
                Err(error) => {
                    self.show_toast(format!("Preset save rejected: {error}"));
                    Ok(None)
                }
                result => result,
            },
            PlatformAction::MidiPanic => {
                self.show_toast("MIDI panic sent");
                self.platform_effect_for_action(&action)
            }
            PlatformAction::MidiCcLearn(slot) => {
                if slot < MIDI_CC_SLOT_COUNT {
                    self.start_midi_cc_learn(slot);
                }
                Ok(None)
            }
            PlatformAction::SystemControlsHelp => {
                self.open_controls_help();
                Ok(None)
            }
            PlatformAction::Signal(source) => {
                self.open_full_screen_signal(source);
                Ok(None)
            }
            PlatformAction::SystemHardwareTest => {
                self.open_hardware_test();
                Ok(None)
            }
            PlatformAction::SystemInfo => {
                self.open_system_info();
                self.platform_effect_for_action(&action)
            }
            PlatformAction::SystemConfigureWifi => {
                self.open_setup_portal();
                Ok(None)
            }
            PlatformAction::SystemClearAll => {
                self.clear_patch_state()?;
                Ok(None)
            }
            PlatformAction::SystemReboot => self.start_power_action(RuntimePlatformEffect::Reboot),
            PlatformAction::SystemShutdown => {
                self.start_power_action(RuntimePlatformEffect::Shutdown)
            }
            PlatformAction::UsbSdTransferStart => {
                self.transport.transport = RuntimeTransportState::Stopped;
                self.reset_transport_position();
                self.display.help_popup = None;
                self.open_usb_sd_transfer_modal();
                self.platform_effect_for_action(&action)
            }
            PlatformAction::MaintenanceHold => {
                self.show_toast("Live Show Mode: 48h");
                self.platform_effect_for_action(&action)
            }
            PlatformAction::MaintenanceRelease => {
                self.show_toast("Live Show Mode off");
                self.platform_effect_for_action(&action)
            }
            PlatformAction::FactoryLoad => {
                self.apply_factory_payload()?;
                Ok(None)
            }
            PlatformAction::FaderTileMap => {
                self.enter_fader_tile_assign();
                Ok(None)
            }
            PlatformAction::PlayFxMap => {
                self.enter_play_fx_assign();
                Ok(None)
            }
            PlatformAction::SampleAssign {
                instrument_slot,
                sample_slot,
            } => {
                self.grid_assign = Some(GridAssign::Sample {
                    slot: instrument_slot,
                    sample_slot,
                });
                self.show_toast(format!("S{}: press tiles to assign", sample_slot + 1));
                Ok(None)
            }
            PlatformAction::DrumAssign { slot, voice } => {
                self.enter_drum_assign(slot, voice);
                Ok(None)
            }
            PlatformAction::MidiDrumAssign(slot) => {
                self.enter_midi_drum_assign(slot);
                Ok(None)
            }
            PlatformAction::DrumPreview { slot, voice } => {
                self.preview_drum_voice(slot, voice);
                Ok(None)
            }
            PlatformAction::TriggerProbabilityAssign(layer_index) => {
                self.grid_assign = Some(GridAssign::TriggerProbability(
                    layer_index.min(GRID_HEIGHT - 1),
                ));
                self.show_toast(format!("L{}: press tiles to assign", layer_index + 1));
                Ok(None)
            }
            PlatformAction::FactoryInstrumentLoad { kind, slot, id } => {
                if let Err(error) = self.load_factory_instrument(kind, slot, &id) {
                    self.show_toast(format!("Factory load rejected: {error}"));
                }
                Ok(None)
            }
            PlatformAction::SampleOpen {
                instrument_slot,
                sample_slot,
                dir,
            } => Ok(Some(self.open_sample_browser(
                instrument_slot,
                sample_slot,
                dir,
            ))),
            PlatformAction::SampleEnter {
                instrument_slot,
                sample_slot,
                dir,
            } => Ok(Some(self.browse_sample_dir(
                instrument_slot,
                sample_slot,
                dir,
            ))),
            PlatformAction::SampleUp {
                instrument_slot,
                sample_slot,
            } => Ok(Some(
                self.browse_sample_parent_dir(instrument_slot, sample_slot),
            )),
            PlatformAction::SamplePick {
                instrument_slot,
                sample_slot,
                path,
            } => {
                self.pick_sample(instrument_slot, sample_slot, path);
                Ok(None)
            }
            PlatformAction::SampleFavoriteSet {
                instrument_slot,
                sample_slot,
            } => self.toggle_sample_favourite(instrument_slot, sample_slot, true),
            PlatformAction::SampleFavoriteRemove {
                instrument_slot,
                sample_slot,
            } => self.toggle_sample_favourite(instrument_slot, sample_slot, false),
            PlatformAction::PresetRefresh
            | PlatformAction::PresetLoad(_)
            | PlatformAction::PresetDelete(_)
            | PlatformAction::DefaultSave
            | PlatformAction::DefaultLoad
            | PlatformAction::SystemSave
            | PlatformAction::SystemLoad
            | PlatformAction::SystemBackupRestore
            | PlatformAction::SystemUpdateCheck
            | PlatformAction::SystemUpdateApply
            | PlatformAction::SystemRollback
            | PlatformAction::UsbSdTransferStop
            | PlatformAction::RecordingStartAudio
            | PlatformAction::RecordingStartAudioOled
            | PlatformAction::RecordingStop
            | PlatformAction::MidiOutput(_)
            | PlatformAction::MidiInput(_)
            | PlatformAction::BluetoothScan
            | PlatformAction::BluetoothToggle(_)
            | PlatformAction::BluetoothPair(_)
            | PlatformAction::BluetoothForget(_) => self.platform_effect_for_action(&action),
        }
    }

    fn enter_play_fx_assign(&mut self) {
        let config = self.play_fx_selected.clone();
        self.grid_assign = Some(GridAssign::PlayFx(config.clone()));
        self.active_play_mode = ActivePlayMode::Page(PlayMode::Fx);
        self.show_toast(format!(
            "FX {}: press tiles to assign",
            super::super::play_control::play_fx_type_label(&config)
        ));
    }

    fn open_setup_portal(&mut self) {
        self.stop_for_setup_portal();
        if !self.wifi_enabled {
            self.wifi_enabled = true;
            self.menu.set_bool_value_for_key("wifi.enabled", true);
            self.mark_system_dirty();
        }
        self.outbox
            .push_platform_effect(RuntimePlatformEffect::MidiPanic);
        self.outbox
            .push_platform_effect(RuntimePlatformEffect::SetupPortalOpen);
    }
}
