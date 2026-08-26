use super::super::oled_error_tests::menu_snapshot;
use super::super::oled_glyph_tests::{glyph_snapshot, GLYPH_FIXTURES};
use serde_json::{json, Value};

pub(super) fn parity_corpus() -> Vec<(String, Value)> {
    let mut cases = Vec::new();
    push(&mut cases, "normal", menu_snapshot());

    let mut unselected = menu_snapshot();
    unselected["selectedRow"] = Value::Null;
    push(&mut cases, "normal-unselected", unselected);

    for (name, fraction) in [("empty-bar", 0.0), ("partial-bar", 0.5), ("full-bar", 1.0)] {
        let mut snapshot = menu_snapshot();
        snapshot["display"]["barValues"][1] = json!({ "frac": fraction });
        push(&mut cases, name, snapshot);
    }
    let mut marker = menu_snapshot();
    marker["display"]["barValues"][1] = json!({ "frac": 0.5, "style": "marker" });
    push(&mut cases, "marker-bar", marker);

    let mut long = menu_snapshot();
    long["display"]["title"] = json!("System Info / Very Long Title");
    long["display"]["lines"][1] = json!("  a very long line that must clip at the OLED bar");
    long["display"]["scrollOffset"] = json!(7);
    long["display"]["totalRows"] = json!(24);
    push(&mut cases, "scrollbar-middle", long);
    let mut scroll_first = menu_snapshot();
    scroll_first["display"]["scrollOffset"] = json!(0);
    push(&mut cases, "scrollbar-first", scroll_first);
    let mut scroll_last = menu_snapshot();
    scroll_last["display"]["scrollOffset"] = json!(5);
    push(&mut cases, "scrollbar-last", scroll_last);

    let mut colors = menu_snapshot();
    colors["display"]["colors"] = json!([
        platform_core::palette::RED_RGB565,
        platform_core::palette::GREEN_RGB565,
        platform_core::palette::BLUE_RGB565,
        platform_core::palette::YELLOW_RGB565,
        platform_core::palette::GRAY_RGB565,
        platform_core::palette::WHITE_RGB565,
        platform_core::palette::BLACK_RGB565
    ]);
    push(&mut cases, "row-colors", colors);

    let mut build_title = menu_snapshot();
    build_title["display"]["title"] = json!("B");
    push(&mut cases, "title-expansion-build", build_title);
    let mut system_title = menu_snapshot();
    system_title["display"]["title"] = json!("/SYS");
    push(&mut cases, "title-expansion-system", system_title);
    let mut editing = menu_snapshot();
    editing["display"]["editing"] = json!(true);
    push(&mut cases, "editing", editing);

    for (name, icon, flash) in [
        ("event-stopped-beat", "stop", "beat"),
        ("event-stopped-measure", "stop", "measure"),
        ("event-paused-beat", "pause", "beat"),
        ("event-paused-measure", "pause", "measure"),
        ("event-playing-beat", "play", "beat"),
        ("event-playing-measure", "play", "measure"),
    ] {
        let mut snapshot = menu_snapshot();
        snapshot["transportIcon"] = json!(icon);
        snapshot["transportFlash"] = json!(flash);
        push(&mut cases, name, snapshot);
    }

    let mut event = menu_snapshot();
    event["eventDotOn"] = json!(true);
    push(&mut cases, "event-dot-default", event);
    let mut voice_steal = menu_snapshot();
    voice_steal["eventDotOn"] = json!(true);
    voice_steal["voiceSteal"] = json!(true);
    push(&mut cases, "voice-steal-dot", voice_steal);

    let mut toast = menu_snapshot();
    toast["display"]["toast"] = json!("Help=Sh+Fn/Enter");
    toast["settings"]["autoSaveFlash"] = json!("flash");
    toast["settings"]["autoSaveFlashSerial"] = json!(12);
    push(&mut cases, "toast-event-flash", toast);
    let mut save = menu_snapshot();
    save["settings"]["autoSaveFlash"] = json!("flash");
    save["settings"]["autoSaveFlashSerial"] = json!(12);
    push(&mut cases, "save-flash", save);
    let mut high_load = menu_snapshot();
    high_load["cpuLoadRatio"] = json!(0.95);
    push(&mut cases, "high-load", high_load);
    let mut cpu_save = menu_snapshot();
    cpu_save["cpuLoadRatio"] = json!(0.95);
    cpu_save["settings"]["autoSaveFlash"] = json!("flash");
    cpu_save["settings"]["autoSaveFlashSerial"] = json!(13);
    push(&mut cases, "cpu-save-together", cpu_save);

    let mut midi_error = menu_snapshot();
    midi_error["display"]["splash"] = json!("startup");
    midi_error["display"]["toast"] = json!("Loading");
    midi_error["runtimeError"] = json!({
        "domain": "midi",
        "code": "operation_failed",
        "operation": "midi_list_inputs",
        "message": "MIDI unavailable"
    });
    push(&mut cases, "concise-midi-error-splash-toast", midi_error);
    let mut runtime_error = menu_snapshot();
    runtime_error["display"]["splash"] = json!("startup");
    runtime_error["display"]["toast"] = json!("Saved");
    runtime_error["runtimeError"] = json!({
        "domain": "sample",
        "code": "not_found",
        "operation": "audio_command",
        "message": "sample not found"
    });
    push(&mut cases, "full-runtime-error-priority", runtime_error);

    for (name, splash) in [
        ("startup-splash", "startup"),
        ("sleep-splash", "sleep"),
        ("shutdown-splash", "shutdown"),
    ] {
        let mut snapshot = menu_snapshot();
        snapshot["display"]["splash"] = json!(splash);
        if splash == "startup" {
            snapshot["display"]["toast"] = json!("Loading");
        }
        push(&mut cases, name, snapshot);
    }
    let mut off = menu_snapshot();
    off["display"]["off"] = json!(true);
    off["display"]["splash"] = json!("startup");
    off["display"]["toast"] = json!("Saved");
    off["runtimeError"] = json!({
        "domain": "sample",
        "code": "not_found",
        "operation": "audio_command",
        "message": "sample not found"
    });
    push(&mut cases, "off-priority-over-all", off);

    for brightness in [0, 50, 100] {
        let mut snapshot = menu_snapshot();
        snapshot["settings"]["displayBrightness"] = json!(brightness);
        push(
            &mut cases,
            &format!("brightness-selected-bar-{brightness}"),
            snapshot,
        );
        let mut error = menu_snapshot();
        error["settings"]["displayBrightness"] = json!(brightness);
        error["runtimeError"] = json!({
            "domain": "sample",
            "code": "not_found",
            "operation": "audio_command",
            "message": "sample not found"
        });
        push(&mut cases, &format!("brightness-error-{brightness}"), error);
        let mut splash = menu_snapshot();
        splash["settings"]["displayBrightness"] = json!(brightness);
        splash["display"]["splash"] = json!("startup");
        push(
            &mut cases,
            &format!("brightness-splash-{brightness}"),
            splash,
        );
        let mut toast = menu_snapshot();
        toast["settings"]["displayBrightness"] = json!(brightness);
        toast["display"]["toast"] = json!("Loading");
        push(&mut cases, &format!("brightness-toast-{brightness}"), toast);
    }

    for fixture in GLYPH_FIXTURES {
        push(&mut cases, fixture.name, glyph_snapshot(fixture));
    }

    let mut empty_colors = menu_snapshot();
    empty_colors["display"]["colors"] = json!([]);
    push(&mut cases, "empty-colors", empty_colors);
    let mut missing_colors = menu_snapshot();
    missing_colors["display"]
        .as_object_mut()
        .unwrap()
        .remove("colors");
    push(&mut cases, "missing-colors", missing_colors);
    let mut empty_bars = menu_snapshot();
    empty_bars["display"]["barValues"] = json!([]);
    push(&mut cases, "empty-bars", empty_bars);
    let mut missing_bars = menu_snapshot();
    missing_bars["display"]
        .as_object_mut()
        .unwrap()
        .remove("barValues");
    push(&mut cases, "missing-bars", missing_bars);
    let mut missing_scroll = menu_snapshot();
    for key in ["scrollOffset", "totalRows", "visibleRows"] {
        missing_scroll["display"]
            .as_object_mut()
            .unwrap()
            .remove(key);
    }
    push(&mut cases, "missing-scroll", missing_scroll);
    let mut empty_scroll = menu_snapshot();
    empty_scroll["display"]["scrollOffset"] = json!(0);
    empty_scroll["display"]["totalRows"] = json!(0);
    empty_scroll["display"]["visibleRows"] = json!(0);
    push(&mut cases, "empty-scroll", empty_scroll);
    let mut empty_lines = menu_snapshot();
    empty_lines["display"]["lines"] = json!([]);
    empty_lines["selectedRow"] = Value::Null;
    push(&mut cases, "empty-display-lines", empty_lines);
    let mut empty_sample_browser = menu_snapshot();
    empty_sample_browser["display"]["title"] = json!("Samples");
    empty_sample_browser["display"]["lines"] = json!(["(empty)"]);
    empty_sample_browser["display"]["colors"] = json!([platform_core::palette::WHITE_RGB565]);
    empty_sample_browser["display"]["barValues"] = json!([null]);
    empty_sample_browser["display"]["scrollOffset"] = Value::Null;
    empty_sample_browser["display"]["totalRows"] = Value::Null;
    empty_sample_browser["display"]["visibleRows"] = Value::Null;
    empty_sample_browser["selectedRow"] = Value::Null;
    push(&mut cases, "empty-sample-browser", empty_sample_browser);

    for (name, title, lines) in [
        (
            "help-shape",
            "Help",
            vec!["Volume", "Tap to change", "> Close"],
        ),
        (
            "confirm-shape",
            "Confirm",
            vec!["Discard changes?", "  Yes", "> No"],
        ),
        (
            "sample-shape",
            "Samples",
            vec!["..", "[drums]", "kick.wav", "(empty)"],
        ),
        (
            "usb-shape",
            "USB Transfer",
            vec!["Copying", "12 files", "> Stop Transfer"],
        ),
        (
            "setup-shape",
            "Wi-Fi Setup",
            vec!["Hotspot:", "Octessera Setup ABCD", "> Close"],
        ),
        (
            "system-info-shape",
            "System Info",
            vec!["Version 0.7.5", "CPU 12%", "> Back"],
        ),
    ] {
        let mut snapshot = menu_snapshot();
        snapshot["display"]["title"] = json!(title);
        snapshot["display"]["lines"] = json!(lines);
        snapshot["display"]["colors"] = json!([
            platform_core::palette::WHITE_RGB565,
            platform_core::palette::WHITE_RGB565,
            platform_core::palette::WHITE_RGB565
        ]);
        snapshot["display"]["barValues"] = json!([null, null, null]);
        snapshot["display"]["scrollOffset"] = Value::Null;
        snapshot["display"]["totalRows"] = Value::Null;
        snapshot["display"]["visibleRows"] = Value::Null;
        snapshot["selectedRow"] = json!(2);
        push(&mut cases, name, snapshot);
    }

    for (name, lines) in [
        ("setup-starting", vec!["Starting hotspot...", "> Hide"]),
        (
            "setup-portal-ready",
            vec![
                "Hotspot:",
                "Octessera Setup 318d",
                "Open 192.168.42.1",
                "Portal: 10 minutes",
                "> Hide",
            ],
        ),
        (
            "setup-portal-ready-transfer",
            vec![
                "Hotspot:",
                "Octessera Setup 318d",
                "Data 192.168.42.1",
                "Port 8081",
                "Code Ab12Cd",
                "Portal: 10 minutes",
                "> Hide",
            ],
        ),
        ("setup-finalizing", vec!["Applying settings...", "> Hide"]),
        (
            "setup-succeeded",
            vec![
                "Setup complete",
                "IP in System > Info",
                "No reboot needed",
                "> Close",
            ],
        ),
        (
            "setup-failed",
            vec!["Setup failed", "Check the device status", "> Close"],
        ),
        (
            "setup-timed-out",
            vec!["Setup timed out", "Portal closed", "> Close"],
        ),
        (
            "setup-unsupported",
            vec!["Not available on", "desktop", "> Close"],
        ),
    ] {
        let mut snapshot = menu_snapshot();
        snapshot["display"]["bodyLayout"] = json!("card");
        snapshot["display"]["title"] = json!("Wi-Fi Setup");
        snapshot["display"]["lines"] = json!(lines);
        snapshot["display"]["colors"] =
            json!(vec![platform_core::palette::WHITE_RGB565; lines.len()]);
        snapshot["display"]["barValues"] = json!(vec![Value::Null; lines.len()]);
        snapshot["display"]["scrollOffset"] = Value::Null;
        snapshot["display"]["totalRows"] = Value::Null;
        snapshot["display"]["visibleRows"] = Value::Null;
        snapshot["selectedRow"] = json!(lines.len() - 1);
        push(&mut cases, name, snapshot);
    }

    let mut long_error = menu_snapshot();
    long_error["runtimeError"] = json!({
        "domain": "sample",
        "code": "operation_failed",
        "operation": "sample_preview",
        "message": "this is a deliberately long diagnostic path that must not escape the OLED card"
    });
    push(&mut cases, "long-runtime-error", long_error);
    let mut long_toast = menu_snapshot();
    long_toast["display"]["toast"] = json!("A deliberately long toast message for the OLED");
    push(&mut cases, "long-toast", long_toast);
    cases
}

fn push(cases: &mut Vec<(String, Value)>, name: &str, snapshot: Value) {
    cases.push((name.to_owned(), snapshot));
}
