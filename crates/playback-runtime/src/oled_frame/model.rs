use super::OledDisplayLayout;
use serde::{Deserialize, Serialize};

pub const INDICATOR_FADE_LEVELS: u8 = 4;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OledPresentationInput {
    pub display: OledDisplayInput,
    pub selected_row: Option<usize>,
    pub transport: OledTransportInput,
    pub event_dot_level: u8,
    pub display_brightness: u8,
    pub cpu_warn_percent: u8,
    pub save_flash: OledSaveFlash,
    pub save_flash_serial: u64,
    pub metrics: OledPresentationMetrics,
    pub runtime_error: Option<OledRuntimeErrorMetadata>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OledDisplayInput {
    pub off: bool,
    pub splash: OledSplash,
    pub body_layout: OledDisplayLayout,
    pub title: String,
    pub title_color: u16,
    pub lines: Vec<String>,
    pub colors: Vec<u16>,
    pub bars: Vec<Option<OledBarInput>>,
    /// Parallel to `lines`: the parts of each menu item row, `None` for plain text lines.
    pub rows: Vec<Option<OledMenuRowInput>>,
    pub scroll: Option<OledScrollInput>,
    pub editing: bool,
    pub toast: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OledMenuRowInput {
    pub kind: OledMenuRowKind,
    pub label: String,
    pub value: String,
    pub text_cursor: Option<usize>,
    pub aux_cap: Option<OledAuxCap>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OledMenuRowKind {
    Value,
    Group,
    Action,
}

/// The Aux encoder (1-3) and layer whose control maps to a row or footer hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OledAuxCap {
    pub encoder: u8,
    pub shift: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OledBarInput {
    pub fraction: f32,
    pub style: OledBarStyle,
}

impl Default for OledBarInput {
    fn default() -> Self {
        Self {
            fraction: 0.0,
            style: OledBarStyle::Fill,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum OledBarStyle {
    #[default]
    Fill,
    Marker,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OledScrollInput {
    pub offset: usize,
    pub total_rows: usize,
    pub visible_rows: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OledTransportInput {
    pub icon: OledTransportIcon,
    pub flash: OledTransportFlash,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum OledTransportIcon {
    Play,
    Pause,
    #[default]
    Stop,
    Other,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum OledTransportFlash {
    #[default]
    None,
    Beat,
    Measure,
    Other,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum OledSaveFlash {
    #[default]
    None,
    Flash,
    Other,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum OledSplash {
    #[default]
    None,
    Boot,
    Sleep,
    Shutdown,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OledPresentationMetrics {
    pub worker_utilization: Option<f32>,
    pub cpu_load_peak_percent: u8,
    pub high_cpu_steady: bool,
    pub missed_quantum_flash: bool,
}

impl OledPresentationMetrics {
    pub fn from_status(
        worker_utilization: Option<f32>,
        high_cpu_steady: bool,
        missed_quantum_flash: bool,
    ) -> Self {
        Self {
            worker_utilization,
            cpu_load_peak_percent: 0,
            high_cpu_steady: worker_utilization.is_some() && high_cpu_steady,
            missed_quantum_flash,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OledRuntimeErrorMetadata {
    pub domain: Option<String>,
    pub code: Option<String>,
    pub operation: Option<String>,
    pub message: Option<String>,
}
