use super::oled::{oled_frame, OLED_FRAME_BYTES};
use super::oled_error_tests::{menu_snapshot, producer_error};
use super::oled_glyph_tests::{
    assert_glyph_fixture_coverage, glyph_snapshot, GLYPH_DIGITS_FNV1A64, GLYPH_FIXTURES,
    GLYPH_LOWERCASE_FIRST_FNV1A64, GLYPH_LOWERCASE_LAST_FNV1A64, GLYPH_PUNCTUATION_FIRST_FNV1A64,
    GLYPH_PUNCTUATION_LAST_FNV1A64, GLYPH_SYMBOLS_FNV1A64, GLYPH_UPPERCASE_FIRST_FNV1A64,
    GLYPH_UPPERCASE_LAST_FNV1A64,
};
use super::oled_test_adapter::input_from_snapshot;
use playback_runtime::oled_frame::{render_oled_frame, test_support};
use serde_json::json;

#[path = "oled_parity_corpus.rs"]
mod parity_corpus;
use parity_corpus::parity_corpus;

const NORMAL_SELECTED_FNV1A64: u64 = 0xAFAB_0247_A45C_1F39;
const TOAST_ACTIVE_EVENT_TRANSPORT_FNV1A64: u64 = 0x176C_78B8_3179_27FF;
const FULL_RUNTIME_ERROR_FNV1A64: u64 = 0x2E40_726F_83B0_07C9;
const CONCISE_MIDI_ERROR_FNV1A64: u64 = 0x247E_FFE5_93FC_3319;
const STARTUP_SPLASH_100_FNV1A64: u64 = 0x0E92_C1C2_4C3B_175B;
const STARTUP_SPLASH_50_FNV1A64: u64 = 0x4AB2_E39F_7B90_AA8E;
const SLEEP_SPLASH_FNV1A64: u64 = 0x0E92_C1C2_4C3B_175B;
const SHUTDOWN_SPLASH_FNV1A64: u64 = SLEEP_SPLASH_FNV1A64;
const OFF_FNV1A64: u64 = 0x8F69_55BF_94EC_2325;
const BOOT_SPLASH_ASSET_FNV1A64: u64 = STARTUP_SPLASH_100_FNV1A64;
const SLEEP_SHUTDOWN_SPLASH_ASSET_FNV1A64: u64 = SLEEP_SPLASH_FNV1A64;

#[test]
fn playback_oled_renderer_matches_pi_reference_corpus() {
    let corpus = parity_corpus();
    assert_eq!(corpus.len(), 75);
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
fn unicode_path_error_keeps_pi_and_playback_pixel_parity() {
    let mut snapshot = menu_snapshot();
    snapshot["runtimeError"] = producer_error(
        playback_runtime::RuntimeErrorDomain::Sample,
        playback_runtime::RuntimeErrorCode::OperationFailed,
        playback_runtime::RuntimeOperation::SamplePreview,
        Some("/音色/very_long_file_abcdefghijklmnabcdefghijklmn"),
    );
    assert_eq!(
        oled_frame(&snapshot),
        render_oled_frame(&input_from_snapshot(&snapshot))
    );
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
