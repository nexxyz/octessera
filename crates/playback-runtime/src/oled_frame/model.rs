use super::OledDisplayLayout;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
    pub aux_hints: OledAuxHints,
    /// Shift is held (alone or with Fn), so the footer shows the Shift-layer hints.
    pub aux_shift_layer: bool,
    /// A System recording is running (the footer's recording disc).
    pub recording_active: bool,
    /// The recording disc's pulse is in its half-brightness phase.
    pub recording_pulse_dim: bool,
    pub metrics: OledPresentationMetrics,
    pub runtime_error: Option<OledRuntimeErrorMetadata>,
    pub animation: OledAnimationInput,
}

/// The in-between state of running OLED animations, resolved when the scene is captured;
/// empty when the screen shows its end state.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OledAnimationInput {
    pub body: Option<OledBodyAnimation>,
    pub toast: Option<OledToastSlide>,
}

impl OledAnimationInput {
    pub fn is_empty(&self) -> bool {
        self.body.is_none() && self.toast.is_none()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum OledBodyAnimation {
    /// The selection bar has moved `moved` of 13 px from `from_row` towards the selected row.
    Select { from_row: usize, moved: usize },
    /// The list has moved `moved` of 13 px from `from`; the bar stays on the selected slot.
    Scroll {
        from: Arc<OledBodyFrom>,
        down: bool,
        moved: usize,
    },
    /// The new body has slid `moved` of 128 px in over `from`, from the right when `forward`.
    Slide {
        from: Arc<OledBodyFrom>,
        forward: bool,
        moved: usize,
    },
    /// The selected row's E2 box is half grown or half shrunk.
    Edit,
}

/// The body a scroll or slide leaves.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OledBodyFrom {
    pub display: OledDisplayInput,
    pub selected_row: Option<usize>,
}

/// The toast box with `visible_rows` of its 13 rows above the bottom edge.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OledToastSlide {
    pub text: String,
    pub visible_rows: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    /// A browser file row whose sample preview is sounding: a play triangle in its marker column.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub preview: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OledMenuRowKind {
    Value,
    Group,
    Action,
    /// A file or folder in a browser list: no marker, and its list has no Aux cap gutter.
    Entry,
}

/// The Aux encoder (1-3) and layer whose control maps to a row or footer hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OledAuxCap {
    pub encoder: u8,
    pub shift: bool,
}

/// Footer hints for Aux 1-3, plain and Shift layer; empty where nothing is mapped or the
/// matching System > Display hint toggle is off.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OledAuxHints {
    pub normal: [OledAuxHint; 3],
    pub shift: [OledAuxHint; 3],
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OledAuxHint {
    pub turn: Option<OledHintLabel>,
    pub click: Option<OledHintLabel>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OledHintLabel {
    /// The full label; the renderer shortens it to the room its footer slot has.
    pub text: String,
    /// RGB565 colour of the menu section that owns the target.
    pub color: u16,
    pub source: OledHintSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OledHintSource {
    Auto,
    Manual,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OledBarStyle {
    #[default]
    Fill,
    Marker,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    pub missed_quantum_flash: bool,
}

impl OledPresentationMetrics {
    pub fn from_status(worker_utilization: Option<f32>, missed_quantum_flash: bool) -> Self {
        Self {
            worker_utilization,
            cpu_load_peak_percent: 0,
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
