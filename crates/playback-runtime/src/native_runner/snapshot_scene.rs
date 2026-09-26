use super::snapshot::display_active_cells;
use super::snapshot_leds::base_led_snapshot;
use super::toast_text::scrolled_toast;
use super::{
    display_transients::DisplayTransientPresentation, GridInteraction, NativeFxBus,
    NativeInstrumentSlot, NativeOledMode, NativeRunner, NativeTransportState, NativeUiState, Value,
    GRID_HEIGHT, GRID_WIDTH,
};
use crate::native_runner::snapshot_display::DisplaySnapshot;
use crate::oled_frame::{
    OledBarInput, OledBarStyle, OledDisplayInput, OledPresentationInput, OledPresentationMetrics,
    OledRuntimeErrorMetadata, OledSaveFlash, OledScrollInput, OledSplash, OledTransportFlash,
    OledTransportIcon, OledTransportInput,
};
use crate::protocol::MidiPort;
use platform_core::{BehaviorRenderModel, GlobalSoundConfig};

#[path = "snapshot_scene_json.rs"]
mod conversion;
#[path = "snapshot_scene_hardware.rs"]
mod hardware;
pub use hardware::{
    NativeControlButtonPresentation, NativeGridPresentation, NativeHardwarePresentation,
    NativeHdmiMode, NativeHdmiPresentation, NativeLedPresentation,
};
#[cfg(test)]
#[path = "snapshot_scene_hardware_tests.rs"]
mod hardware_tests;

struct AudioScene {
    instruments: Vec<NativeInstrumentSlot>,
    buses: Vec<NativeFxBus>,
    slots: Vec<String>,
    params: Vec<Value>,
}

enum HdmiGrid {
    Black,
    Live,
    Model(BehaviorRenderModel),
}

struct HdmiScene {
    mode: String,
    show_gridlines: bool,
    cycle_measures: u8,
    source_layer_index: usize,
    source_behavior_id: String,
    grid: HdmiGrid,
}

pub struct PresentationScene {
    generation: u64,
    display: DisplaySnapshot,
    behavior_id: String,
    toast: String,
    off: bool,
    splash: String,
    editing: bool,
    led_rgb: Vec<u8>,
    active_cells: Vec<bool>,
    hdmi: HdmiScene,
    transport: NativeTransportState,
    play_mode: String,
    active_play_mode: String,
    momentary_grid: bool,
    ui: NativeUiState,
    sound: GlobalSoundConfig,
    voice_stealing_mode: String,
    input_events_while_paused: bool,
    leds_dimmed: bool,
    aux_auto_map_enabled: bool,
    audio_config_revision: u64,
    auto_save_flash: bool,
    auto_save_flash_serial: u64,
    midi_enabled: bool,
    selected_midi_output_id: Option<String>,
    selected_midi_input_id: Option<String>,
    midi_outputs: Vec<MidiPort>,
    midi_inputs: Vec<MidiPort>,
    midi_status: Option<String>,
    midi_clock_out_enabled: bool,
    midi_clock_in_enabled: bool,
    midi_respond_to_start_stop: bool,
    neo_key_colors: [[u8; 3]; 4],
    transient: DisplayTransientPresentation,
    audio: Option<AudioScene>,
}

impl NativeRunner {
    pub(super) fn capture_presentation_scene(
        &self,
        include_audio_config: bool,
    ) -> Result<PresentationScene, String> {
        #[cfg(any(test, feature = "test-support"))]
        if self.test_snapshot_failure.replace(false) {
            return Err("test snapshot construction failure".into());
        }
        let transient = self
            .display
            .transients
            .presentation(self.display.transients.now());
        let model = self.engine.model()?;
        let active_cells = display_active_cells(&model.cells);
        let mut leds = base_led_snapshot(&model);
        self.apply_scan_progress_overlay(&mut leds);
        self.apply_play_overlay(&mut leds);
        if self.play_fx_assign.is_none() {
            if self.sample_assign.is_some() {
                self.apply_sample_assignment_overlay(&mut leds);
            } else if self.drum_assign.is_some() || self.drum_cell_tune.is_some() {
                self.apply_drum_assignment_overlay(&mut leds);
            } else if self.trigger_probability_assign.is_some() {
                self.apply_trigger_probability_overlay(&mut leds);
            }
        }
        self.apply_param_mod_overlay(&mut leds);
        self.apply_fn_overlay(&mut leds);
        let mut led_rgb = Vec::with_capacity(GRID_WIDTH * GRID_HEIGHT * 3);
        for led in leds {
            led.append_rgb(&mut led_rgb);
        }
        let hdmi_mode = self.display.hdmi.mode.as_str();
        let source_layer_index = self.hdmi_source_layer_index(hdmi_mode);
        let source_behavior_id = self
            .layer_behavior_ids
            .get(source_layer_index)
            .cloned()
            .unwrap_or_else(|| "none".into());
        let grid = match hdmi_mode {
            "live-grid" => HdmiGrid::Live,
            "plain-grid" => HdmiGrid::Model(model),
            "active-behavior" | "cycle-behaviors"
                if source_layer_index == self.active_layer_index
                    && source_behavior_id != "none" =>
            {
                HdmiGrid::Model(model)
            }
            "active-behavior" | "cycle-behaviors" => self
                .hdmi_model_for_layer(source_layer_index)
                .map(HdmiGrid::Model)
                .unwrap_or(HdmiGrid::Black),
            _ => HdmiGrid::Black,
        };
        let display = self.display_snapshot(self.menu.snapshot());
        let audio = include_audio_config.then(|| AudioScene {
            instruments: self.instruments.clone(),
            buses: self.fx_buses.clone(),
            slots: self.global_fx_slots.clone(),
            params: self.global_fx_params.clone(),
        });
        Ok(PresentationScene {
            generation: self.display.transients.generation(),
            display,
            behavior_id: self.behavior.id().to_string(),
            toast: self
                .display
                .toast
                .as_ref()
                .map(scrolled_toast)
                .unwrap_or_default(),
            off: self.display.oled_mode == NativeOledMode::Off,
            splash: if self.display.runtime_error_presentation.is_none()
                && self.display.oled_mode == NativeOledMode::Splash
            {
                self.display.oled_splash_text.clone()
            } else {
                String::new()
            },
            editing: self.menu.state.editing && self.display.help_popup.is_none(),
            led_rgb,
            active_cells,
            hdmi: HdmiScene {
                mode: self.display.hdmi.mode.clone(),
                show_gridlines: self.display.hdmi.show_gridlines,
                cycle_measures: self.display.hdmi.cycle_measures,
                source_layer_index,
                source_behavior_id,
                grid,
            },
            transport: self.transport.clone(),
            play_mode: self.play_mode.clone(),
            active_play_mode: self.active_play_mode.clone(),
            momentary_grid: self
                .behavior
                .grid_interaction()
                .unwrap_or(GridInteraction::Paint)
                == GridInteraction::Momentary,
            ui: self.display.ui.clone(),
            sound: self.global_sound.clone(),
            voice_stealing_mode: self.voice_stealing_mode.clone(),
            input_events_while_paused: self.input_events_while_paused,
            leds_dimmed: self.leds_dimmed(),
            aux_auto_map_enabled: self.aux_auto_map_enabled,
            audio_config_revision: self.audio_config_revision,
            auto_save_flash: self.auto_save_flash_active(),
            auto_save_flash_serial: self.display.auto_save_flash_serial,
            midi_enabled: self.midi_enabled,
            selected_midi_output_id: self.selected_midi_output_id.clone(),
            selected_midi_input_id: self.selected_midi_input_id.clone(),
            midi_outputs: self.midi_outputs.clone(),
            midi_inputs: self.midi_inputs.clone(),
            midi_status: self.midi_status.clone(),
            midi_clock_out_enabled: self.midi_clock_out_enabled,
            midi_clock_in_enabled: self.midi_clock_in_enabled,
            midi_respond_to_start_stop: self.midi_respond_to_start_stop,
            neo_key_colors: self.neo_key_colors(transient),
            transient,
            audio,
        })
    }
}

impl PresentationScene {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn oled_presentation_input(
        &self,
        metrics: OledPresentationMetrics,
        runtime_error: Option<OledRuntimeErrorMetadata>,
    ) -> OledPresentationInput {
        let display = &self.display;
        OledPresentationInput {
            display: OledDisplayInput {
                off: self.off,
                splash: match self.splash.as_str() {
                    "" => OledSplash::None,
                    "sleep" => OledSplash::Sleep,
                    "shutdown" => OledSplash::Shutdown,
                    _ => OledSplash::Boot,
                },
                body_layout: display.body_layout,
                title: display.title.clone(),
                lines: display.lines.clone(),
                colors: display.colors.clone(),
                bars: display
                    .bar_values
                    .iter()
                    .map(|bar| {
                        bar.as_ref().map(|bar| OledBarInput {
                            fraction: f32::from(bar.frac_pct) / 100.0,
                            style: if bar.style.as_deref() == Some("marker") {
                                OledBarStyle::Marker
                            } else {
                                OledBarStyle::Fill
                            },
                        })
                    })
                    .collect(),
                scroll: display.scroll.as_ref().map(|scroll| OledScrollInput {
                    offset: scroll.scroll_offset,
                    total_rows: scroll.total_rows,
                    visible_rows: scroll.visible_rows,
                }),
                editing: self.editing,
                toast: self.toast.clone(),
            },
            selected_row: display.selected_row,
            transport: OledTransportInput {
                icon: match self.transport.transport {
                    super::RuntimeTransportState::Playing => OledTransportIcon::Play,
                    super::RuntimeTransportState::Paused => OledTransportIcon::Pause,
                    super::RuntimeTransportState::Stopped => OledTransportIcon::Stop,
                },
                flash: match self.transient.transport_flash {
                    super::TransportFlash::None => OledTransportFlash::None,
                    super::TransportFlash::Beat => OledTransportFlash::Beat,
                    super::TransportFlash::Measure => OledTransportFlash::Measure,
                },
            },
            event_dot_on: self.transient.event_dot_on,
            display_brightness: self.ui.display_brightness,
            save_flash: if self.auto_save_flash {
                OledSaveFlash::Flash
            } else {
                OledSaveFlash::None
            },
            save_flash_serial: self.auto_save_flash_serial,
            metrics,
            runtime_error,
        }
    }
}
