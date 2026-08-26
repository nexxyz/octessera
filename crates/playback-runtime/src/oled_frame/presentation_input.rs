use super::{
    OledBarInput, OledBarStyle, OledDisplayInput, OledDisplayLayout, OledPresentationInput,
    OledPresentationMetrics, OledRuntimeErrorMetadata, OledSaveFlash, OledScrollInput, OledSplash,
    OledTransportFlash, OledTransportIcon, OledTransportInput,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OledPresentationInputError {
    pub(crate) field: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DisplayInputDto {
    off: bool,
    splash: String,
    #[serde(rename = "bodyLayout")]
    body_layout: OledDisplayLayout,
    title: String,
    lines: Vec<String>,
    colors: Vec<u16>,
    #[serde(rename = "barValues")]
    bars: Vec<Option<BarInputDto>>,
    #[serde(rename = "scrollOffset")]
    scroll_offset: Option<usize>,
    #[serde(rename = "totalRows")]
    total_rows: Option<usize>,
    #[serde(rename = "visibleRows")]
    visible_rows: Option<usize>,
    editing: bool,
    toast: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BarInputDto {
    frac: f32,
    #[serde(default)]
    style: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OledSettingsDto {
    display_brightness: u8,
    auto_save_flash: String,
    auto_save_flash_serial: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OledErrorDto {
    #[serde(default)]
    domain: Option<String>,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    operation: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

pub(crate) fn presentation_input_from_snapshot(
    snapshot: &Value,
    metrics: OledPresentationMetrics,
) -> Result<Option<OledPresentationInput>, OledPresentationInputError> {
    let Some(snapshot) = snapshot.as_object() else {
        return Err(missing("snapshot"));
    };
    if !snapshot.contains_key("display") && !snapshot.contains_key("settings") {
        return Ok(None);
    }
    let display = required::<DisplayInputDto>(snapshot, "display")?;
    let settings = required::<OledSettingsDto>(snapshot, "settings")?;
    let selected_row = required_nullable_usize(snapshot, "selectedRow")?;
    let event_dot_on = required_bool(snapshot, "eventDotOn")?;
    let transport_icon = required_string(snapshot, "transportIcon")?;
    let transport_flash = required_string(snapshot, "transportFlash")?;
    let runtime_error =
        optional::<OledErrorDto>(snapshot, "runtimeError")?.map(|error| OledRuntimeErrorMetadata {
            domain: error.domain,
            code: error.code,
            operation: error.operation,
            message: error.message,
        });

    let scroll = match (
        display.scroll_offset,
        display.total_rows,
        display.visible_rows,
    ) {
        (Some(offset), Some(total_rows), Some(visible_rows)) => Some(OledScrollInput {
            offset,
            total_rows,
            visible_rows,
        }),
        (None, None, None) => None,
        _ => return Err(missing("display.scrollOffset/totalRows/visibleRows")),
    };
    let bars = display
        .bars
        .into_iter()
        .map(|bar| {
            bar.map(|bar| OledBarInput {
                fraction: bar.frac,
                style: match bar.style.as_deref() {
                    Some("marker") => OledBarStyle::Marker,
                    _ => OledBarStyle::Fill,
                },
            })
        })
        .collect();

    Ok(Some(OledPresentationInput {
        display: OledDisplayInput {
            off: display.off,
            splash: splash_from_name(&display.splash),
            body_layout: display.body_layout,
            title: display.title,
            lines: display.lines,
            colors: display.colors,
            bars,
            scroll,
            editing: display.editing,
            toast: display.toast,
        },
        selected_row,
        transport: OledTransportInput {
            icon: transport_icon_from_name(&transport_icon),
            flash: transport_flash_from_name(&transport_flash),
        },
        event_dot_on,
        display_brightness: settings.display_brightness,
        save_flash: save_flash_from_name(&settings.auto_save_flash),
        save_flash_serial: settings.auto_save_flash_serial,
        metrics,
        runtime_error,
    }))
}

fn required<T: for<'de> Deserialize<'de>>(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<T, OledPresentationInputError> {
    let value = object.get(field).ok_or_else(|| missing(field))?;
    serde_json::from_value(value.clone()).map_err(|_| missing(field))
}

fn optional<T: for<'de> Deserialize<'de>>(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<Option<T>, OledPresentationInputError> {
    let Some(value) = object.get(field) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    serde_json::from_value(value.clone())
        .map(Some)
        .map_err(|_| missing(field))
}

fn required_bool(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<bool, OledPresentationInputError> {
    object
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| missing(field))
}

fn required_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<String, OledPresentationInputError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| missing(field))
}

fn required_nullable_usize(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<Option<usize>, OledPresentationInputError> {
    let value = object.get(field).ok_or_else(|| missing(field))?;
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .map(Some)
        .ok_or_else(|| missing(field))
}

fn missing(field: &str) -> OledPresentationInputError {
    OledPresentationInputError {
        field: field.to_owned(),
    }
}

fn splash_from_name(value: &str) -> OledSplash {
    match value {
        "" => OledSplash::None,
        "sleep" => OledSplash::Sleep,
        "shutdown" => OledSplash::Shutdown,
        _ => OledSplash::Boot,
    }
}

fn save_flash_from_name(value: &str) -> OledSaveFlash {
    match value {
        "none" => OledSaveFlash::None,
        "flash" => OledSaveFlash::Flash,
        _ => OledSaveFlash::Other,
    }
}

fn transport_icon_from_name(value: &str) -> OledTransportIcon {
    match value {
        "play" => OledTransportIcon::Play,
        "pause" => OledTransportIcon::Pause,
        "stop" => OledTransportIcon::Stop,
        _ => OledTransportIcon::Other,
    }
}

fn transport_flash_from_name(value: &str) -> OledTransportFlash {
    match value {
        "none" => OledTransportFlash::None,
        "beat" => OledTransportFlash::Beat,
        "measure" => OledTransportFlash::Measure,
        _ => OledTransportFlash::Other,
    }
}
