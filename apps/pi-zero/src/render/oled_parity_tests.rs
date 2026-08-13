use super::oled::{oled_frame, OLED_FRAME_BYTES};
use super::oled_error_tests::menu_snapshot;
use super::oled_glyph_tests::{
    assert_glyph_fixture_coverage, glyph_snapshot, GLYPH_DIGITS_FNV1A64, GLYPH_FIXTURES,
    GLYPH_LOWERCASE_FIRST_FNV1A64, GLYPH_LOWERCASE_LAST_FNV1A64, GLYPH_PUNCTUATION_FIRST_FNV1A64,
    GLYPH_PUNCTUATION_LAST_FNV1A64, GLYPH_SYMBOLS_FNV1A64, GLYPH_UPPERCASE_FIRST_FNV1A64,
    GLYPH_UPPERCASE_LAST_FNV1A64,
};
use super::oled_test_adapter::input_from_snapshot;
use playback_runtime::oled_frame::{render_oled_frame, test_support};
use serde_json::{json, Value};

const NORMAL_SELECTED_FNV1A64: u64 = 0xAFAB_0247_A45C_1F39;
const TOAST_ACTIVE_EVENT_TRANSPORT_FNV1A64: u64 = 0x176C_78B8_3179_27FF;
const FULL_RUNTIME_ERROR_FNV1A64: u64 = 0xB732_4E42_2F8A_B1C9;
const CONCISE_MIDI_ERROR_FNV1A64: u64 = 0x247E_FFE5_93FC_3319;
const STARTUP_SPLASH_100_FNV1A64: u64 = 0x0E92_C1C2_4C3B_175B;
const STARTUP_SPLASH_50_FNV1A64: u64 = 0x4AB2_E39F_7B90_AA8E;
const SLEEP_SPLASH_FNV1A64: u64 = 0xB27A_BF03_4D41_F9B9;
const SHUTDOWN_SPLASH_FNV1A64: u64 = SLEEP_SPLASH_FNV1A64;
const OFF_FNV1A64: u64 = 0x8F69_55BF_94EC_2325;
const BOOT_SPLASH_ASSET_FNV1A64: u64 = STARTUP_SPLASH_100_FNV1A64;
const SLEEP_SHUTDOWN_SPLASH_ASSET_FNV1A64: u64 = SLEEP_SPLASH_FNV1A64;

#[test]
fn playback_oled_renderer_matches_pi_reference_corpus() {
    let corpus = parity_corpus();
    assert_eq!(corpus.len(), 65);
    for (name, snapshot) in corpus {
        let pi_frame = oled_frame(&snapshot);
        let playback_frame = render_oled_frame(&input_from_snapshot(&snapshot));
        assert_eq!(pi_frame.len(), OLED_FRAME_BYTES, "Pi frame length: {name}");
        assert_eq!(
            playback_frame.len(),
            OLED_FRAME_BYTES,
            "Playback frame length: {name}"
        );
        assert_eq!(playback_frame, pi_frame, "OLED parity case: {name}");
    }
}

#[test]
fn pi_and_playback_frozen_frames_have_stable_anchors() {
    let mut full_error = menu_snapshot();
    full_error["display"]["splash"] = json!("startup");
    full_error["display"]["toast"] = json!("Saved");
    full_error["runtimeError"] = json!({
        "domain": "sample",
        "code": "not_found",
        "operation": "audio_command",
        "message": "sample not found"
    });
    let mut concise_midi = menu_snapshot();
    concise_midi["display"]["title"] = json!("MIDI INPUTS");
    concise_midi["display"]["lines"] = json!(["MIDI unavailable"]);
    concise_midi["display"]["colors"] = json!([platform_core::palette::WHITE_RGB565]);
    concise_midi["display"]["barValues"] = json!([null]);
    concise_midi["runtimeError"] = json!({
        "domain": "midi",
        "code": "operation_failed",
        "operation": "midi_list_inputs",
        "message": "MIDI unavailable"
    });
    let mut startup_50 = menu_snapshot();
    startup_50["display"]["splash"] = json!("startup");
    startup_50["settings"]["displayBrightness"] = json!(50);
    let mut startup_100 = startup_50.clone();
    startup_100["settings"]["displayBrightness"] = json!(100);
    let mut sleep = menu_snapshot();
    sleep["display"]["splash"] = json!("sleep");
    let mut shutdown = menu_snapshot();
    shutdown["display"]["splash"] = json!("shutdown");
    let mut off = menu_snapshot();
    off["display"]["off"] = json!(true);
    off["display"]["splash"] = json!("startup");
    off["display"]["toast"] = json!("Saved");
    off["runtimeError"] = full_error["runtimeError"].clone();
    let mut toast = menu_snapshot();
    toast["display"]["toast"] = json!("Help=Sh+Fn/Enter");

    for (name, snapshot, expected) in [
        ("normal-selected", menu_snapshot(), NORMAL_SELECTED_FNV1A64),
        (
            "toast-active-event-transport",
            toast,
            TOAST_ACTIVE_EVENT_TRANSPORT_FNV1A64,
        ),
        (
            "full-runtime-error",
            menu_snapshot(),
            FULL_RUNTIME_ERROR_FNV1A64,
        ),
        (
            "concise-midi-error",
            menu_snapshot(),
            CONCISE_MIDI_ERROR_FNV1A64,
        ),
        (
            "startup-splash-brightness-100",
            startup_100,
            STARTUP_SPLASH_100_FNV1A64,
        ),
        (
            "startup-splash-brightness-50",
            startup_50,
            STARTUP_SPLASH_50_FNV1A64,
        ),
        ("sleep-splash", sleep, SLEEP_SPLASH_FNV1A64),
        ("shutdown-splash", shutdown, SHUTDOWN_SPLASH_FNV1A64),
        ("off", off, OFF_FNV1A64),
    ] {
        let snapshot = match name {
            "full-runtime-error" => full_error.clone(),
            "concise-midi-error" => concise_midi.clone(),
            _ => snapshot,
        };
        let pi_frame = oled_frame(&snapshot);
        let playback_frame = render_oled_frame(&input_from_snapshot(&snapshot));
        assert_eq!(pi_frame.len(), OLED_FRAME_BYTES, "Pi frame length: {name}");
        assert_eq!(
            playback_frame.len(),
            OLED_FRAME_BYTES,
            "Playback frame length: {name}"
        );
        assert_eq!(pi_frame, playback_frame, "OLED parity case: {name}");
        assert_eq!(fnv1a64(&pi_frame), expected, "frozen frame anchor: {name}");
    }

    assert_glyph_fixture_coverage();
    for fixture in GLYPH_FIXTURES {
        let frame = oled_frame(&glyph_snapshot(fixture));
        let expected = match fixture.name {
            "glyph-digits" => GLYPH_DIGITS_FNV1A64,
            "glyph-uppercase-first" => GLYPH_UPPERCASE_FIRST_FNV1A64,
            "glyph-uppercase-last" => GLYPH_UPPERCASE_LAST_FNV1A64,
            "glyph-lowercase-first" => GLYPH_LOWERCASE_FIRST_FNV1A64,
            "glyph-lowercase-last" => GLYPH_LOWERCASE_LAST_FNV1A64,
            "glyph-punctuation-first" => GLYPH_PUNCTUATION_FIRST_FNV1A64,
            "glyph-punctuation-last" => GLYPH_PUNCTUATION_LAST_FNV1A64,
            "glyph-symbols" => GLYPH_SYMBOLS_FNV1A64,
            _ => unreachable!("unlisted glyph fixture: {}", fixture.name),
        };
        assert_eq!(
            frame.len(),
            OLED_FRAME_BYTES,
            "frame length: {}",
            fixture.name
        );
        assert_eq!(
            fnv1a64(&frame),
            expected,
            "frozen frame anchor: {}",
            fixture.name
        );
    }

    assert_eq!(super::SPLASH_BOOT.len(), OLED_FRAME_BYTES);
    assert_eq!(super::SPLASH_SLEEP_SHUTDOWN.len(), OLED_FRAME_BYTES);
    assert_eq!(test_support::BOOT_SPLASH_RGB565.len(), OLED_FRAME_BYTES);
    assert_eq!(
        test_support::SLEEP_SHUTDOWN_SPLASH_RGB565.len(),
        OLED_FRAME_BYTES
    );
    assert_eq!(fnv1a64(super::SPLASH_BOOT), BOOT_SPLASH_ASSET_FNV1A64);
    assert_eq!(
        fnv1a64(super::SPLASH_SLEEP_SHUTDOWN),
        SLEEP_SHUTDOWN_SPLASH_ASSET_FNV1A64
    );
    assert_eq!(super::SPLASH_BOOT, test_support::BOOT_SPLASH_RGB565);
    assert_eq!(
        super::SPLASH_SLEEP_SHUTDOWN,
        test_support::SLEEP_SHUTDOWN_SPLASH_RGB565
    );
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    })
}

fn parity_corpus() -> Vec<(String, Value)> {
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
    cases
}

fn push(cases: &mut Vec<(String, Value)>, name: &str, snapshot: Value) {
    cases.push((name.to_owned(), snapshot));
}
