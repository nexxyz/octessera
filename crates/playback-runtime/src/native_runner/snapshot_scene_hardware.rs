use super::{HdmiGrid, HdmiScene, PresentationScene};
use crate::native_runner::snapshot::hdmi_frame_from_model;
use crate::oled_frame::{OledPresentationInput, OledPresentationMetrics, OledRuntimeErrorMetadata};
use platform_core::{GRID_HEIGHT, GRID_WIDTH};

#[derive(Debug, PartialEq)]
pub struct NativeHardwarePresentation {
    pub generation: u64,
    pub oled: OledPresentationInput,
    pub leds: NativeLedPresentation,
    pub neo_key: NativeControlButtonPresentation,
    pub hdmi: NativeHdmiPresentation,
}

#[derive(Debug, PartialEq, Eq)]
pub struct NativeGridPresentation {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<u8>,
    pub active: Vec<bool>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct NativeLedPresentation {
    pub grid: NativeGridPresentation,
    pub brightness: u8,
    pub dimmed: bool,
    pub off: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct NativeControlButtonPresentation {
    pub colors: [[u8; 3]; 4],
    pub brightness: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeHdmiMode {
    None,
    LiveGrid,
    PlainGrid,
    ActiveBehavior,
    CycleBehaviors,
}

impl NativeHdmiMode {
    fn from_name(name: &str) -> Self {
        match name {
            "none" => Self::None,
            "live-grid" => Self::LiveGrid,
            "plain-grid" => Self::PlainGrid,
            "active-behavior" => Self::ActiveBehavior,
            "cycle-behaviors" => Self::CycleBehaviors,
            _ => panic!("invalid canonical HDMI mode: {name}"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::LiveGrid => "live-grid",
            Self::PlainGrid => "plain-grid",
            Self::ActiveBehavior => "active-behavior",
            Self::CycleBehaviors => "cycle-behaviors",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct NativeHdmiPresentation {
    pub mode: NativeHdmiMode,
    pub show_gridlines: bool,
    pub cycle_measures: u8,
    pub source_layer_index: usize,
    pub source_behavior_id: String,
    pub grid: NativeGridPresentation,
}

impl HdmiGrid {
    pub(super) fn into_frame(self, live_rgb: &[u8], live_active: &[bool]) -> (Vec<u8>, Vec<bool>) {
        match self {
            Self::Black => (
                vec![0; GRID_WIDTH * GRID_HEIGHT * 3],
                vec![false; GRID_WIDTH * GRID_HEIGHT],
            ),
            Self::Live => (live_rgb.to_vec(), live_active.to_vec()),
            Self::Model(model) => hdmi_frame_from_model(&model),
        }
    }
}

impl HdmiScene {
    fn into_hardware(self, live_rgb: &[u8], live_active: &[bool]) -> NativeHdmiPresentation {
        let (rgb, active) = self.grid.into_frame(live_rgb, live_active);
        NativeHdmiPresentation {
            mode: NativeHdmiMode::from_name(&self.mode),
            show_gridlines: self.show_gridlines,
            cycle_measures: self.cycle_measures,
            source_layer_index: self.source_layer_index,
            source_behavior_id: self.source_behavior_id,
            grid: NativeGridPresentation {
                width: GRID_WIDTH,
                height: GRID_HEIGHT,
                rgb,
                active,
            },
        }
    }
}

impl PresentationScene {
    pub fn into_hardware_presentation(
        self,
        metrics: OledPresentationMetrics,
        runtime_error: Option<OledRuntimeErrorMetadata>,
    ) -> NativeHardwarePresentation {
        let oled = self.oled_presentation_input(metrics, runtime_error);
        let hdmi = self.hdmi.into_hardware(&self.led_rgb, &self.active_cells);
        NativeHardwarePresentation {
            generation: self.generation,
            oled,
            leds: NativeLedPresentation {
                grid: NativeGridPresentation {
                    width: GRID_WIDTH,
                    height: GRID_HEIGHT,
                    rgb: self.led_rgb,
                    active: self.active_cells,
                },
                brightness: self.ui.grid_brightness,
                dimmed: self.leds_dimmed,
                off: self.off,
            },
            neo_key: NativeControlButtonPresentation {
                colors: self.neo_key_colors,
                brightness: self.ui.button_brightness,
            },
            hdmi,
        }
    }
}
