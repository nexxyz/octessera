use super::OledDisplayLayout;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OledPresentationInput {
    pub display: OledDisplayInput,
    pub selected_row: Option<usize>,
    pub transport: OledTransportInput,
    pub event_dot_on: bool,
    pub display_brightness: u8,
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
    pub lines: Vec<String>,
    pub colors: Vec<u16>,
    pub bars: Vec<Option<OledBarInput>>,
    pub scroll: Option<OledScrollInput>,
    pub editing: bool,
    pub toast: String,
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OledPresentationMetrics {
    pub cpu_hot: bool,
    pub voice_steal: bool,
}

impl OledPresentationMetrics {
    pub fn normalized(audio_load_ratio: f32, voice_steal: bool) -> Self {
        Self {
            cpu_hot: audio_load_ratio.is_finite() && audio_load_ratio >= 0.85,
            voice_steal,
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
