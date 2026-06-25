use super::*;
use serde_json::{json, Value};

fn pixel(frame: &[u8], x: usize, y: usize) -> u16 {
    let idx = (y * 128 + x) * 2;
    u16::from_be_bytes([frame[idx], frame[idx + 1]])
}

fn menu_snapshot() -> Value {
    json!({
        "display": {
            "off": false,
            "splash": "",
            "title": "Voice FX/Aux",
            "lines": ["  Volume +3", "@@ FX/Aux 1", "*Velocity", "  sample_1", "  (empty)", "  Q/V X", "  J+K"],
            "colors": [65535, 2016, 65535, 65535, 65535, 65535, 65535],
            "barValues": [null, { "frac": 0.5 }, null, null, null, null, null],
            "scrollOffset": 2,
            "totalRows": 12,
            "visibleRows": 7,
            "toast": "",
            "editing": false
        },
        "settings": { "displayBrightness": 100, "autoSaveFlash": "none", "autoSaveFlashSerial": 0 },
        "selectedRow": 1,
        "transportIcon": "play",
        "transportFlash": "beat",
        "eventDotOn": true,
        "cpuLoadRatio": 0.7
    })
}

#[test]
fn oled_frame_renders_menu_bars_selection_status_and_scrollbar() {
    let frame = oled_frame(&menu_snapshot());
    assert_ne!(pixel(&frame, 5, 5), 0);
    assert_eq!(pixel(&frame, 4, 30), 2016);
    assert_eq!(pixel(&frame, 88, 33), 2016);
    assert_ne!(pixel(&frame, 125, 18), 0);
    assert_ne!(pixel(&frame, 102, 118), 0);
    assert_ne!(pixel(&frame, 119, 119), 0);
}

#[test]
fn oled_frame_into_matches_allocating_renderer() {
    let snapshot = menu_snapshot();
    let expected = oled_frame(&snapshot);
    let mut reused = vec![0xa5_u8; OLED_FRAME_BYTES];
    oled_frame_into(&snapshot, &mut reused);
    assert_eq!(reused, expected);
}

#[test]
fn oled_frame_into_clears_reused_buffer() {
    let mut snapshot = menu_snapshot();
    let mut reused = vec![0xff_u8; OLED_FRAME_BYTES];
    snapshot["display"]["off"] = json!(true);
    oled_frame_into(&snapshot, &mut reused);
    assert!(reused.iter().all(|byte| *byte == 0));
}

#[test]
fn oled_frame_into_clears_between_splash_menu_and_off() {
    let mut snapshot = menu_snapshot();
    let mut reused = vec![0xa5_u8; OLED_FRAME_BYTES];
    snapshot["display"]["splash"] = json!("sleep");
    oled_frame_into(&snapshot, &mut reused);
    assert!(reused.iter().any(|byte| *byte != 0));

    snapshot["display"]["splash"] = json!("");
    oled_frame_into(&snapshot, &mut reused);
    assert_eq!(reused, oled_frame(&snapshot));

    snapshot["display"]["off"] = json!(true);
    oled_frame_into(&snapshot, &mut reused);
    assert!(reused.iter().all(|byte| *byte == 0));
}

#[test]
fn glyphs_cover_common_menu_sample_and_help_text() {
    for ch in "FX/AUX VELOCITY J+Q/V X (EMPTY) SAMPLE_1 #3".chars() {
        if ch != ' ' {
            assert_ne!(glyph_rows(ch), [0; 7], "missing glyph {ch}");
        }
    }
}

#[test]
fn toast_footer_has_priority_over_transport_and_event_dot() {
    let mut snapshot = menu_snapshot();
    snapshot["display"]["toast"] = json!("Help=Sh+Fn/Enter");
    let frame = oled_frame(&snapshot);
    assert_ne!(pixel(&frame, 5, 118), 0);
    assert_eq!(pixel(&frame, 119, 119), rgb565([6, 18, 13]));
}

#[test]
fn oled_signature_tracks_scroll_bar_status_and_float_changes() {
    let snapshot = menu_snapshot();
    let base = oled_signature(&snapshot);
    let mut changed = snapshot.clone();
    changed["display"]["barValues"][1]["frac"] = json!(0.75);
    assert_ne!(base, oled_signature(&changed));
    changed = snapshot.clone();
    changed["display"]["scrollOffset"] = json!(3);
    assert_ne!(base, oled_signature(&changed));
    changed = snapshot.clone();
    changed["cpuLoadRatio"] = json!(0.71);
    assert_ne!(base, oled_signature(&changed));
}
