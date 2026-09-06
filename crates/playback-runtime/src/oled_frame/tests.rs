use super::error_layout::runtime_error_rows;
use super::model::*;
use super::pixels::rgb565;
use super::render::render_oled_frame;
use super::splash::{SPLASH_BOOT, SPLASH_SLEEP_SHUTDOWN};
use super::OledDisplayLayout;
use super::OLED_FRAME_BYTES;
use platform_core::palette;

const NORMAL_SELECTED_FNV1A64: u64 = 0xAFAB_0247_A45C_1F39;
const TOAST_ACTIVE_EVENT_TRANSPORT_FNV1A64: u64 = 0x176C_78B8_3179_27FF;
const FULL_RUNTIME_ERROR_FNV1A64: u64 = 0x2E40_726F_83B0_07C9;
const CONCISE_MIDI_ERROR_FNV1A64: u64 = 0x247E_FFE5_93FC_3319;
const STARTUP_SPLASH_100_FNV1A64: u64 = 0x0E92_C1C2_4C3B_175B;
const STARTUP_SPLASH_50_FNV1A64: u64 = 0x4AB2_E39F_7B90_AA8E;
const SLEEP_SPLASH_FNV1A64: u64 = 0x0E92_C1C2_4C3B_175B;
const SHUTDOWN_SPLASH_FNV1A64: u64 = SLEEP_SPLASH_FNV1A64;
const OFF_FNV1A64: u64 = 0x8F69_55BF_94EC_2325;
const GLYPH_DIGITS_FNV1A64: u64 = 0x9FE1_8ECD_428C_BC3F;
const GLYPH_UPPERCASE_FIRST_FNV1A64: u64 = 0x81AD_DF18_3C6F_3865;
const GLYPH_UPPERCASE_LAST_FNV1A64: u64 = 0x4D26_9228_40F7_11A5;
const GLYPH_LOWERCASE_FIRST_FNV1A64: u64 = 0xD1AA_9D54_9F8E_8067;
const GLYPH_LOWERCASE_LAST_FNV1A64: u64 = 0x1472_784E_157A_10A5;
const GLYPH_PUNCTUATION_FIRST_FNV1A64: u64 = 0x67F4_248A_C2B9_8D85;
const GLYPH_PUNCTUATION_LAST_FNV1A64: u64 = 0x1662_9A94_8669_A771;
const GLYPH_SYMBOLS_FNV1A64: u64 = 0x9416_2B3D_DEEE_A9EF;
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
        ("glyph-digits", glyph_fixture("0123456789")),
        (
            "glyph-uppercase-first",
            glyph_fixture("ABCDEFGHIJKLMNOPQRS"),
        ),
        ("glyph-uppercase-last", glyph_fixture("TUVWXYZ")),
        (
            "glyph-lowercase-first",
            glyph_fixture("abcdefghijklmnopqrs"),
        ),
        ("glyph-lowercase-last", glyph_fixture("tuvwxyz")),
        (
            "glyph-punctuation-first",
            glyph_fixture(":.-*+/()_#@><[]%!"),
        ),
        ("glyph-punctuation-last", glyph_fixture("?,\'\"=")),
        ("glyph-symbols", glyph_fixture("▶■●|")),
    ];
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
        assert_eq!(fnv1a64(&frame), expected, "frame anchor: {name}");
    }
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
fn missed_quantum_flash_inverts_cpu_slot_over_high_cpu_and_preserves_save_icon() {
    let mut high_cpu = base();
    high_cpu.metrics = OledPresentationMetrics::from_status(Some(0.9), true, false, false);
    let high_cpu_frame = render_oled_frame(&high_cpu);
    assert_eq!(pixel(&high_cpu_frame, 118, 6), rgb565(palette::RED));

    let mut missed = high_cpu;
    missed.metrics.missed_quantum_flash = true;
    missed.save_flash = OledSaveFlash::Flash;
    let missed_frame = render_oled_frame(&missed);

    assert_eq!(pixel(&missed_frame, 120, 8), palette::WHITE_RGB565);
    assert_eq!(pixel(&missed_frame, 118, 6), palette::BLACK_RGB565);
    assert_eq!(pixel(&missed_frame, 107, 5), palette::YELLOW_RGB565);
    assert_ne!(missed_frame, high_cpu_frame);
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
    input.event_dot_on = true;
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
    input.display.lines = [text, text, text, text, text, text, text]
        .into_iter()
        .map(str::to_owned)
        .collect();
    input.display.colors = vec![platform_core::palette::WHITE_RGB565; 7];
    input.display.bars = vec![None; 7];
    input.selected_row = None;
    input
}

fn base() -> OledPresentationInput {
    OledPresentationInput {
        display: OledDisplayInput {
            body_layout: OledDisplayLayout::Rows,
            title: "Voice FX/Aux".into(),
            lines: [
                "  Volume +3",
                "@@ FX/Aux 1",
                "*Velocity",
                "  sample_1",
                "  (empty)",
                "  Q/V X",
                "  J+K",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            colors: vec![
                platform_core::palette::WHITE_RGB565,
                platform_core::palette::GREEN_RGB565,
                platform_core::palette::WHITE_RGB565,
                platform_core::palette::WHITE_RGB565,
                platform_core::palette::WHITE_RGB565,
                platform_core::palette::WHITE_RGB565,
                platform_core::palette::WHITE_RGB565,
            ],
            bars: vec![
                None,
                Some(OledBarInput {
                    fraction: 0.5,
                    style: OledBarStyle::Fill,
                }),
                None,
                None,
                None,
                None,
                None,
            ],
            scroll: Some(OledScrollInput {
                offset: 2,
                total_rows: 12,
                visible_rows: 7,
            }),
            ..Default::default()
        },
        selected_row: Some(1),
        transport: OledTransportInput {
            icon: OledTransportIcon::Play,
            flash: OledTransportFlash::Beat,
        },
        event_dot_on: true,
        display_brightness: 100,
        metrics: OledPresentationMetrics::from_status(None, false, false, false),
        ..Default::default()
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    })
}

fn pixel(frame: &[u8], x: usize, y: usize) -> u16 {
    let offset = (y * 128 + x) * 2;
    u16::from_be_bytes([frame[offset], frame[offset + 1]])
}
