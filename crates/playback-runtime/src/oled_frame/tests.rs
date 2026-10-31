use super::error_layout::runtime_error_rows;
use super::model::*;
use super::pixels::rgb565;
use super::render::render_oled_frame;
use super::splash::{SPLASH_BOOT, SPLASH_SLEEP_SHUTDOWN};
use super::OledDisplayLayout;
use super::INDICATOR_FADE_LEVELS;
use super::OLED_FRAME_BYTES;
use platform_core::palette;

const NORMAL_SELECTED_FNV1A64: u64 = 0x648C_8E6E_E26C_E6D7;
const TOAST_ACTIVE_EVENT_TRANSPORT_FNV1A64: u64 = 0x82BC_890A_3481_E5EC;
const FULL_RUNTIME_ERROR_FNV1A64: u64 = 0x2E40_726F_83B0_07C9;
const CONCISE_MIDI_ERROR_FNV1A64: u64 = 0x6F99_C1ED_A3B3_827C;
const STARTUP_SPLASH_100_FNV1A64: u64 = 0x0E92_C1C2_4C3B_175B;
const STARTUP_SPLASH_50_FNV1A64: u64 = 0x4AB2_E39F_7B90_AA8E;
const SLEEP_SPLASH_FNV1A64: u64 = 0x0E92_C1C2_4C3B_175B;
const SHUTDOWN_SPLASH_FNV1A64: u64 = SLEEP_SPLASH_FNV1A64;
const OFF_FNV1A64: u64 = 0x8F69_55BF_94EC_2325;
const GLYPH_DIGITS_FNV1A64: u64 = 0xAC1C_A2AF_64C3_3660;
const GLYPH_UPPERCASE_FIRST_FNV1A64: u64 = 0xE569_2D5C_7D99_84D0;
const GLYPH_UPPERCASE_LAST_FNV1A64: u64 = 0xFD41_7EA3_F0FA_4180;
const GLYPH_LOWERCASE_FIRST_FNV1A64: u64 = 0x38FC_1612_340F_D8E0;
const GLYPH_LOWERCASE_LAST_FNV1A64: u64 = 0x28D3_743B_CFDE_3BF0;
const GLYPH_PUNCTUATION_FIRST_FNV1A64: u64 = 0x91A3_6F66_811E_4B70;
const GLYPH_PUNCTUATION_LAST_FNV1A64: u64 = 0x1147_090F_8100_5BB0;
const GLYPH_SYMBOLS_FNV1A64: u64 = 0x7DF5_486C_AD3E_AF30;
pub(super) const GLYPH_GOLDEN_TEXTS: [&str; 8] = [
    "0123456789",
    "ABCDEFGHIJKLM",
    "NOPQRSTUVWXYZ",
    "abcdefghijklm",
    "nopqrstuvwxyz",
    "!\"#$%&'()*+,-./",
    ":;<=>?@[\\]^_`{|}~",
    "★ Samples",
];
const BOOT_SPLASH_ASSET_FNV1A64: u64 = STARTUP_SPLASH_100_FNV1A64;
const SLEEP_SHUTDOWN_SPLASH_ASSET_FNV1A64: u64 = SLEEP_SPLASH_FNV1A64;

#[test]
fn typed_oled_golden_anchors_are_stable() {
    let cases = [
        ("normal-selected", normal_selected()),
        (
            "toast-active-event-transport",
            toast_active_event_transport(),
        ),
        ("full-runtime-error", full_runtime_error()),
        ("concise-midi-error", concise_midi_error()),
        ("startup-splash-brightness-100", startup_splash(100)),
        ("startup-splash-brightness-50", startup_splash(50)),
        ("sleep-splash", sleep_splash()),
        ("shutdown-splash", shutdown_splash()),
        ("off", off()),
        ("glyph-digits", glyph_fixture(GLYPH_GOLDEN_TEXTS[0])),
        (
            "glyph-uppercase-first",
            glyph_fixture(GLYPH_GOLDEN_TEXTS[1]),
        ),
        ("glyph-uppercase-last", glyph_fixture(GLYPH_GOLDEN_TEXTS[2])),
        (
            "glyph-lowercase-first",
            glyph_fixture(GLYPH_GOLDEN_TEXTS[3]),
        ),
        ("glyph-lowercase-last", glyph_fixture(GLYPH_GOLDEN_TEXTS[4])),
        (
            "glyph-punctuation-first",
            glyph_fixture(GLYPH_GOLDEN_TEXTS[5]),
        ),
        (
            "glyph-punctuation-last",
            glyph_fixture(GLYPH_GOLDEN_TEXTS[6]),
        ),
        ("glyph-symbols", glyph_fixture(GLYPH_GOLDEN_TEXTS[7])),
    ];
    let mut mismatches = Vec::new();
    for (name, input) in cases {
        let frame = render_oled_frame(&input);
        assert_eq!(frame.len(), OLED_FRAME_BYTES, "frame length: {name}");
        let expected = match name {
            "normal-selected" => NORMAL_SELECTED_FNV1A64,
            "toast-active-event-transport" => TOAST_ACTIVE_EVENT_TRANSPORT_FNV1A64,
            "full-runtime-error" => FULL_RUNTIME_ERROR_FNV1A64,
            "concise-midi-error" => CONCISE_MIDI_ERROR_FNV1A64,
            "startup-splash-brightness-100" => STARTUP_SPLASH_100_FNV1A64,
            "startup-splash-brightness-50" => STARTUP_SPLASH_50_FNV1A64,
            "sleep-splash" => SLEEP_SPLASH_FNV1A64,
            "shutdown-splash" => SHUTDOWN_SPLASH_FNV1A64,
            "off" => OFF_FNV1A64,
            "glyph-digits" => GLYPH_DIGITS_FNV1A64,
            "glyph-uppercase-first" => GLYPH_UPPERCASE_FIRST_FNV1A64,
            "glyph-uppercase-last" => GLYPH_UPPERCASE_LAST_FNV1A64,
            "glyph-lowercase-first" => GLYPH_LOWERCASE_FIRST_FNV1A64,
            "glyph-lowercase-last" => GLYPH_LOWERCASE_LAST_FNV1A64,
            "glyph-punctuation-first" => GLYPH_PUNCTUATION_FIRST_FNV1A64,
            "glyph-punctuation-last" => GLYPH_PUNCTUATION_LAST_FNV1A64,
            "glyph-symbols" => GLYPH_SYMBOLS_FNV1A64,
            _ => unreachable!("unlisted OLED golden case: {name}"),
        };
        if fnv1a64(&frame) != expected {
            mismatches.push(format!("{name}: {:#018X}", fnv1a64(&frame)));
        }
    }
    assert!(
        mismatches.is_empty(),
        "frame anchors changed: {mismatches:#?}"
    );
    assert_eq!(SPLASH_BOOT.len(), OLED_FRAME_BYTES);
    assert_eq!(SPLASH_SLEEP_SHUTDOWN.len(), OLED_FRAME_BYTES);
    assert_eq!(SPLASH_SLEEP_SHUTDOWN, SPLASH_BOOT);
    assert_eq!(fnv1a64(SPLASH_BOOT), BOOT_SPLASH_ASSET_FNV1A64);
    assert_eq!(
        fnv1a64(SPLASH_SLEEP_SHUTDOWN),
        SLEEP_SHUTDOWN_SPLASH_ASSET_FNV1A64
    );
}

#[test]
fn runtime_error_fallback_and_path_punctuation_are_visible_in_the_fixed_font() {
    let mut input = base();
    input.runtime_error = Some(OledRuntimeErrorMetadata {
        domain: Some("sample".into()),
        code: Some("not_found".into()),
        operation: Some("sample_preview".into()),
        message: Some("Ω_/x-y.wav".into()),
    });
    assert_eq!(
        runtime_error_rows(input.runtime_error.as_ref().unwrap())[3],
        "MSG ?_/x-y.wav"
    );

    let frame = render_oled_frame(&input);
    let text = rgb565(palette::GRAY);
    assert_eq!(pixel(&frame, 35, 70), text);
    assert_eq!(pixel(&frame, 40, 76), text);
    assert!((46..=50).any(|x| (70..77).any(|y| pixel(&frame, x, y) == text)));
    assert!((58..=62).any(|x| (70..77).any(|y| pixel(&frame, x, y) == text)));
    assert!((70..=74).any(|x| (70..77).any(|y| pixel(&frame, x, y) == text)));
}

#[test]
fn header_icons_sit_left_of_the_cpu_meter_on_the_crumb_line() {
    let mut input = base();
    input.metrics.missed_quantum_flash = true;
    input.save_flash = OledSaveFlash::Flash;
    let frame = render_oled_frame(&input);
    let red = rgb565(palette::BEHAVIOR_PRIMARY_RED);
    assert_eq!(pixel(&frame, 106, 0), red);
    assert_eq!(pixel(&frame, 108, 2), red);
    assert_eq!(pixel(&frame, 107, 1), 0);
    assert_eq!(pixel(&frame, 110, 4), red);
    for (x, y) in [(117, 0), (116, 1), (113, 2), (115, 2), (113, 3), (114, 4)] {
        assert_eq!(pixel(&frame, x, y), palette::GREEN_RGB565, "check {x},{y}");
    }
    assert_eq!(pixel(&frame, 114, 2), 0);
    assert!((0..128).all(|x| (5..8).all(|y| x < 104 || pixel(&frame, x, y) == 0)));

    let plain = render_oled_frame(&base());
    assert!((104..120).all(|x| (0..8).all(|y| pixel(&plain, x, y) == 0)));
}

#[test]
fn runtime_error_dismissal_affordance_stays_inside_the_error_card() {
    let frame = render_oled_frame(&full_runtime_error());
    let text = rgb565(palette::GRAY);
    let footer_pixels = (0..128)
        .flat_map(|y| (0..128).map(move |x| (x, y)))
        .filter(|(x, y)| *y >= 114 && pixel(&frame, *x, *y) == text)
        .collect::<Vec<_>>();
    assert!(!footer_pixels.is_empty());
    assert!(footer_pixels
        .iter()
        .all(|(x, y)| (4..=123).contains(x) && (114..=123).contains(y)));
    assert!((0..128).all(|x| pixel(&frame, x, 124) != text));
}

#[test]
fn card_layout_ignores_row_bars_and_scroll_metadata() {
    let mut with_metadata = base();
    with_metadata.display.body_layout = OledDisplayLayout::Card;
    with_metadata.display.lines = vec!["Setup complete".into(), "> Close".into()];
    with_metadata.selected_row = Some(1);
    let mut clean = with_metadata.clone();
    with_metadata.display.bars = vec![Some(OledBarInput::default()), None];
    with_metadata.display.scroll = Some(OledScrollInput {
        offset: 1,
        total_rows: 8,
        visible_rows: 7,
    });
    clean.display.bars = vec![None, None];
    clean.display.scroll = None;
    assert_eq!(render_oled_frame(&with_metadata), render_oled_frame(&clean));
}

fn normal_selected() -> OledPresentationInput {
    base()
}

fn toast_active_event_transport() -> OledPresentationInput {
    let mut input = base();
    input.display.toast = "Help=Sh+Fn/Enter".into();
    input.event_dot_level = INDICATOR_FADE_LEVELS;
    input
}

fn full_runtime_error() -> OledPresentationInput {
    let mut input = base();
    input.runtime_error = Some(OledRuntimeErrorMetadata {
        domain: Some("sample".into()),
        code: Some("not_found".into()),
        operation: Some("audio_command".into()),
        message: Some("sample not found".into()),
    });
    input
}

fn concise_midi_error() -> OledPresentationInput {
    let mut input = base();
    input.display.title = "MIDI INPUTS".into();
    input.display.lines = vec!["MIDI unavailable".into()];
    input.display.colors = vec![platform_core::palette::WHITE_RGB565];
    input.display.bars = vec![None];
    input.display.rows = vec![None];
    input.runtime_error = Some(OledRuntimeErrorMetadata {
        domain: Some("midi".into()),
        code: Some("operation_failed".into()),
        operation: Some("midi_list_inputs".into()),
        message: Some("MIDI unavailable".into()),
    });
    input
}

fn startup_splash(brightness: u8) -> OledPresentationInput {
    let mut input = base();
    input.display.splash = OledSplash::Boot;
    input.display_brightness = brightness;
    input
}

fn sleep_splash() -> OledPresentationInput {
    let mut input = base();
    input.display.splash = OledSplash::Sleep;
    input
}

fn shutdown_splash() -> OledPresentationInput {
    let mut input = base();
    input.display.splash = OledSplash::Shutdown;
    input
}

fn off() -> OledPresentationInput {
    let mut input = base();
    input.display.off = true;
    input
}

fn glyph_fixture(text: &str) -> OledPresentationInput {
    let mut input = base();
    input.display.title = "Glyphs".into();
    input.display.lines = vec![text.to_owned(); 8];
    input.display.colors = vec![platform_core::palette::WHITE_RGB565; 8];
    input.display.bars = vec![None; 8];
    input.display.rows = vec![None; 8];
    input.display.scroll = None;
    input.selected_row = None;
    input
}

pub(super) const AREA: u16 = palette::BLUE_RGB565;

pub(super) fn menu_row(
    kind: OledMenuRowKind,
    label: &str,
    value: &str,
    cap: Option<(u8, bool)>,
) -> Option<OledMenuRowInput> {
    Some(OledMenuRowInput {
        kind,
        label: label.into(),
        value: value.into(),
        text_cursor: None,
        aux_cap: cap.map(|(encoder, shift)| OledAuxCap { encoder, shift }),
    })
}

pub(super) fn hint(text: &str) -> Option<OledHintLabel> {
    Some(OledHintLabel {
        text: text.into(),
        color: AREA,
        source: OledHintSource::Auto,
    })
}

/// A Filter page: eight rows of every kind, Cutoff selected, scrolled, three turn hints.
pub(super) fn base() -> OledPresentationInput {
    use OledMenuRowKind::{Action, Group, Value};
    let rows = vec![
        menu_row(Value, "Type", "lowpass", Some((2, true))),
        menu_row(Value, "Cutoff", "221", Some((1, false))),
        menu_row(Value, "Res", "0", Some((2, false))),
        menu_row(Value, "Env Amount", "0%", Some((3, false))),
        menu_row(Value, "Key Tracking", "0%", None),
        menu_row(Group, "Synth", "", None),
        menu_row(Action, "Map to Grid", "", None),
        menu_row(Value, "Name", "Synth", None),
    ];
    let bar = |fraction, style| Some(OledBarInput { fraction, style });
    OledPresentationInput {
        display: OledDisplayInput {
            body_layout: OledDisplayLayout::Rows,
            title: "../I1: synth/Synth/Filter".into(),
            title_color: AREA,
            lines: rows
                .iter()
                .map(|row| row.as_ref().unwrap().label.clone())
                .collect(),
            colors: vec![AREA; rows.len()],
            bars: vec![
                None,
                bar(0.87, OledBarStyle::Fill),
                bar(0.0, OledBarStyle::Fill),
                bar(0.5, OledBarStyle::Marker),
                None,
                None,
                None,
                None,
            ],
            rows,
            scroll: Some(OledScrollInput {
                offset: 2,
                total_rows: 12,
                visible_rows: 8,
            }),
            ..Default::default()
        },
        selected_row: Some(1),
        transport: OledTransportInput {
            icon: OledTransportIcon::Play,
            flash: OledTransportFlash::Beat,
        },
        event_dot_level: INDICATOR_FADE_LEVELS,
        display_brightness: 100,
        cpu_warn_percent: 85,
        aux_hints: OledAuxHints {
            normal: [
                OledAuxHint {
                    turn: hint("Cutoff"),
                    click: None,
                },
                OledAuxHint {
                    turn: hint("Res"),
                    click: None,
                },
                OledAuxHint {
                    turn: hint("Env"),
                    click: None,
                },
            ],
            ..Default::default()
        },
        metrics: OledPresentationMetrics::from_status(None, false),
        ..Default::default()
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    })
}

pub(super) fn pixel(frame: &[u8], x: usize, y: usize) -> u16 {
    let offset = (y * 128 + x) * 2;
    u16::from_be_bytes([frame[offset], frame[offset + 1]])
}
