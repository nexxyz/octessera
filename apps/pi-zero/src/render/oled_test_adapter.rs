use playback_runtime::oled_frame::{
    OledBarInput, OledBarStyle, OledDisplayInput, OledDisplayLayout, OledPresentationInput,
    OledPresentationMetrics, OledRuntimeErrorMetadata, OledSaveFlash, OledScrollInput, OledSplash,
    OledTransportFlash, OledTransportIcon, OledTransportInput,
};
use serde_json::Value;

pub(super) fn input_from_snapshot(snapshot: &Value) -> OledPresentationInput {
    let display = snapshot.get("display").unwrap_or(&Value::Null);
    let settings = snapshot.get("settings").unwrap_or(&Value::Null);
    let splash = match display.get("splash").and_then(Value::as_str) {
        Some("") | None => OledSplash::None,
        Some("sleep") => OledSplash::Sleep,
        Some("shutdown") => OledSplash::Shutdown,
        Some(_) => OledSplash::Boot,
    };
    let bars = display
        .get("barValues")
        .and_then(Value::as_array)
        .map(|bars| {
            bars.iter()
                .map(|bar| {
                    let fraction = bar.get("frac").and_then(Value::as_f64)? as f32;
                    let style = match bar.get("style").and_then(Value::as_str) {
                        Some("marker") => OledBarStyle::Marker,
                        _ => OledBarStyle::Fill,
                    };
                    Some(OledBarInput { fraction, style })
                })
                .collect()
        })
        .unwrap_or_default();
    let scroll = scroll_input(display);
    let runtime_error = snapshot
        .get("runtimeError")
        .map(|error| OledRuntimeErrorMetadata {
            domain: string_field(error, "domain"),
            code: string_field(error, "code"),
            operation: string_field(error, "operation"),
            message: string_field(error, "message"),
        });

    OledPresentationInput {
        display: OledDisplayInput {
            off: bool_field(display, "off"),
            splash,
            body_layout: match display.get("bodyLayout").and_then(Value::as_str) {
                Some("rows") => OledDisplayLayout::Rows,
                Some("card") => OledDisplayLayout::Card,
                _ => panic!("required display.bodyLayout"),
            },
            title: string_field(display, "title").unwrap_or_default(),
            lines: display
                .get("lines")
                .and_then(Value::as_array)
                .map(|lines| {
                    lines
                        .iter()
                        .map(|line| line.as_str().unwrap_or_default().to_owned())
                        .collect()
                })
                .unwrap_or_default(),
            colors: display
                .get("colors")
                .and_then(Value::as_array)
                .map(|colors| {
                    colors
                        .iter()
                        .map(|color| {
                            color
                                .as_u64()
                                .unwrap_or(u64::from(u16::MAX))
                                .min(u64::from(u16::MAX)) as u16
                        })
                        .collect()
                })
                .unwrap_or_default(),
            bars,
            scroll,
            editing: bool_field(display, "editing"),
            toast: string_field(display, "toast").unwrap_or_default(),
        },
        selected_row: snapshot
            .get("selectedRow")
            .and_then(Value::as_u64)
            .map(|row| row as usize),
        transport: OledTransportInput {
            icon: transport_icon(snapshot),
            flash: transport_flash(snapshot),
        },
        event_dot_on: bool_field(snapshot, "eventDotOn"),
        display_brightness: number_field(settings, "displayBrightness", 100).min(100) as u8,
        save_flash: match string_field(settings, "autoSaveFlash").as_deref() {
            Some("flash") => OledSaveFlash::Flash,
            Some(_) => OledSaveFlash::Other,
            None => OledSaveFlash::None,
        },
        save_flash_serial: number_field(settings, "autoSaveFlashSerial", 0),
        metrics: OledPresentationMetrics::normalized(
            snapshot
                .get("cpuLoadRatio")
                .and_then(Value::as_f64)
                .unwrap_or(0.0) as f32,
            bool_field(snapshot, "voiceSteal"),
        ),
        runtime_error,
    }
}

fn scroll_input(display: &Value) -> Option<OledScrollInput> {
    let has_scroll = ["scrollOffset", "totalRows", "visibleRows"]
        .iter()
        .any(|key| display.get(*key).is_some());
    has_scroll.then(|| OledScrollInput {
        offset: number_field(display, "scrollOffset", 0) as usize,
        total_rows: number_field(display, "totalRows", 0) as usize,
        visible_rows: number_field(display, "visibleRows", 0) as usize,
    })
}

fn transport_icon(snapshot: &Value) -> OledTransportIcon {
    match string_field(snapshot, "transportIcon").as_deref() {
        Some("play") => OledTransportIcon::Play,
        Some("pause") => OledTransportIcon::Pause,
        Some("stop") | None => OledTransportIcon::Stop,
        Some(_) => OledTransportIcon::Other,
    }
}

fn transport_flash(snapshot: &Value) -> OledTransportFlash {
    match string_field(snapshot, "transportFlash").as_deref() {
        Some("beat") => OledTransportFlash::Beat,
        Some("measure") => OledTransportFlash::Measure,
        Some("none") | None => OledTransportFlash::None,
        Some(_) => OledTransportFlash::Other,
    }
}

fn bool_field(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn number_field(value: &Value, key: &str, default: u64) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(default)
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}
