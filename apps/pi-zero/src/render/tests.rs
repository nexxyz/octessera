use super::*;
use octessera_hal::OledSsd1351;
use platform_core::palette;
use serde_json::{json, Value};
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn pixel(frame: &[u8], x: usize, y: usize) -> u16 {
    let idx = (y * 128 + x) * 2;
    u16::from_be_bytes([frame[idx], frame[idx + 1]])
}

fn rgb565_to_rgb(value: u16) -> [u8; 3] {
    [
        ((((value >> 11) & 0x1f) * 255) / 31) as u8,
        ((((value >> 5) & 0x3f) * 255) / 63) as u8,
        (((value & 0x1f) * 255) / 31) as u8,
    ]
}

fn menu_snapshot() -> Value {
    json!({
        "display": {
            "off": false,
            "splash": "",
            "title": "Voice FX/Aux",
            "lines": ["  Volume +3", "@@ FX/Aux 1", "*Velocity", "  sample_1", "  (empty)", "  Q/V X", "  J+K"],
            "colors": [
                palette::WHITE_RGB565,
                palette::GREEN_RGB565,
                palette::WHITE_RGB565,
                palette::WHITE_RGB565,
                palette::WHITE_RGB565,
                palette::WHITE_RGB565,
                palette::WHITE_RGB565
            ],
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
        "cpuLoadRatio": 0.0
    })
}

fn snapshot_with_leds() -> Value {
    let mut rgb = Vec::new();
    for _ in 0..64 {
        rgb.extend(palette::YELLOW);
    }
    json!({
        "display": { "off": false },
        "settings": { "gridBrightness": 50, "buttonBrightness": 100 },
        "leds": { "rgb": rgb },
        "transport": { "playing": false },
        "transportIcon": "stop",
        "transportFlash": "none"
    })
}

fn bar_snapshot(frac: f64, style: Option<&str>, selected_row: usize) -> Value {
    let mut snapshot = menu_snapshot();
    snapshot["display"]["barValues"][1] = match style {
        Some(style) => json!({ "frac": frac, "style": style }),
        None => json!({ "frac": frac }),
    };
    snapshot["selectedRow"] = json!(selected_row);
    snapshot
}

#[test]
fn oled_frame_renders_menu_bars_selection_status_and_scrollbar() {
    let frame = oled_frame(&menu_snapshot());
    assert_ne!(pixel(&frame, 5, 5), 0);
    assert_eq!(pixel(&frame, 4, 30), palette::GREEN_RGB565);
    assert_eq!(pixel(&frame, 87, 30), rgb565(palette::BLACK));
    assert_eq!(pixel(&frame, 88, 31), palette::GREEN_RGB565);
    assert_ne!(pixel(&frame, 125, 18), 0);
    assert_ne!(pixel(&frame, 102, 118), 0);
    assert_ne!(pixel(&frame, 119, 119), 0);
}

#[test]
fn oled_bars_render_empty_partial_full_marker_and_selected_contrast() {
    let empty = oled_frame(&bar_snapshot(0.0, None, 0));
    assert_eq!(pixel(&empty, 87, 30), palette::GREEN_RGB565);
    assert_eq!(pixel(&empty, 88, 31), rgb565(dim(palette::GREEN, 6)));

    let partial = oled_frame(&bar_snapshot(0.5, None, 0));
    assert_eq!(pixel(&partial, 88, 31), palette::GREEN_RGB565);
    assert_eq!(pixel(&partial, 105, 31), rgb565(dim(palette::GREEN, 6)));

    let full = oled_frame(&bar_snapshot(1.0, None, 0));
    assert_eq!(pixel(&full, 87, 30), palette::GREEN_RGB565);
    assert_eq!(pixel(&full, 121, 31), palette::GREEN_RGB565);

    let marker = oled_frame(&bar_snapshot(0.5, Some("marker"), 0));
    assert_eq!(pixel(&marker, 105, 32), palette::GREEN_RGB565);
    assert_eq!(pixel(&marker, 104, 32), rgb565(dim(palette::GREEN, 6)));

    let selected = oled_frame(&bar_snapshot(1.0, None, 1));
    assert_eq!(pixel(&selected, 87, 30), rgb565(palette::BLACK));
    assert_eq!(pixel(&selected, 88, 31), palette::GREEN_RGB565);
    assert_eq!(pixel(&selected, 121, 31), palette::GREEN_RGB565);
}

#[test]
fn oled_title_expands_direct_root_short_breadcrumbs() {
    assert_eq!(oled::title_text_for_oled("B"), "/Build");
    assert_eq!(oled::title_text_for_oled("/B"), "/Build");
    assert_eq!(oled::title_text_for_oled("/B/L1: life"), "/B/L1: life");
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
    for ch in "Voice FX/Aux sample_1 (empty) Swing % Help=Sh+Fn/Enter 1!Map ▶■●".chars() {
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
    assert_eq!(pixel(&frame, 119, 119), rgb565(palette::BLACK));
}

#[test]
fn runtime_error_frame_has_priority_over_splash_and_footer() {
    let mut snapshot = menu_snapshot();
    snapshot["display"]["splash"] = json!("startup");
    snapshot["display"]["toast"] = json!("Saved");
    snapshot["runtimeError"] = json!({
        "domain": "sample",
        "code": "not_found",
        "operation": "audio_command",
        "recovery": "retain_last_good",
        "message": "sample not found"
    });

    let frame = oled_frame(&snapshot);

    assert_eq!(pixel(&frame, 0, 0), rgb565(dim(palette::RED, 6)));
    assert_eq!(pixel(&frame, 10, 10), palette::RED_RGB565);
    assert_eq!(pixel(&frame, 119, 119), rgb565(palette::BLACK));
}

#[test]
fn runtime_error_signature_changes_when_error_identity_changes() {
    let mut snapshot = menu_snapshot();
    snapshot["runtimeError"] = json!({
        "domain": "sample",
        "code": "not_found",
        "operation": "audio_command"
    });
    let first = oled_signature(&snapshot);
    snapshot["runtimeError"]["revision"] = json!(4);
    assert_ne!(first, oled_signature(&snapshot));
}

#[test]
fn status_icons_are_invisible_until_warning_or_save_flash() {
    let snapshot = menu_snapshot();
    let frame = oled_frame(&snapshot);
    assert_eq!(pixel(&frame, 118, 6), 0);
    assert_eq!(pixel(&frame, 107, 5), 0);

    let mut high_cpu = snapshot.clone();
    high_cpu["cpuLoadRatio"] = json!(0.9);
    let frame = oled_frame(&high_cpu);
    assert_eq!(pixel(&frame, 118, 6), palette::RED_RGB565);

    let mut saving = snapshot.clone();
    saving["settings"]["autoSaveFlash"] = json!("flash");
    saving["settings"]["autoSaveFlashSerial"] = json!(1);
    let frame = oled_frame(&saving);
    assert_eq!(pixel(&frame, 107, 5), palette::YELLOW_RGB565);
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
    changed["cpuLoadRatio"] = json!(0.9);
    assert_ne!(base, oled_signature(&changed));
}

#[test]
fn led_frame_applies_grid_brightness_and_sleep_dim() {
    let mut snapshot = snapshot_with_leds();
    let frame = led_frame(&snapshot).unwrap();
    assert_eq!(frame[0], scale(palette::YELLOW, 0.5));

    snapshot["display"]["off"] = json!(true);
    let display_off = led_frame(&snapshot).unwrap();
    assert_eq!(display_off[0], scale(palette::YELLOW, 0.5));

    snapshot["settings"]["ledsDimmed"] = json!(true);
    let dimmed = led_frame(&snapshot).unwrap();
    assert_eq!(dimmed[0], scale(palette::YELLOW, 0.04));

    snapshot["settings"]["gridBrightness"] = json!(10);
    let low_brightness_dimmed = led_frame(&snapshot).unwrap();
    assert_eq!(low_brightness_dimmed[0], scale(palette::YELLOW, 0.04));
}

#[test]
fn neokey_play_button_uses_transport_state_and_flash_colors() {
    let mut snapshot = snapshot_with_leds();
    assert_eq!(neokey_colors(&snapshot)[1], palette::RED);

    snapshot["transportIcon"] = json!("pause");
    assert_eq!(neokey_colors(&snapshot)[1], palette::BLUE);

    snapshot["transportIcon"] = json!("play");
    assert_eq!(neokey_colors(&snapshot)[1], dim(palette::GREEN, 3));

    snapshot["transportFlash"] = json!("beat");
    assert_eq!(neokey_colors(&snapshot)[1], palette::YELLOW);

    snapshot["transportFlash"] = json!("measure");
    assert_eq!(neokey_colors(&snapshot)[1], palette::GREEN);

    snapshot["transportFlash"] = json!("none");
    snapshot["display"]["off"] = json!(true);
    assert_eq!(neokey_colors(&snapshot)[1], dim(palette::GREEN, 3));

    snapshot["settings"]["ledsDimmed"] = json!(true);
    assert_eq!(
        neokey_colors(&snapshot)[1],
        scale(dim(palette::GREEN, 3), 0.08)
    );

    snapshot["settings"]["buttonBrightness"] = json!(10);
    assert_eq!(
        neokey_colors(&snapshot)[1],
        scale(dim(palette::GREEN, 3), 0.04)
    );
}

#[test]
fn oled_display_brightness_scales_menu_line_colors() {
    let mut snapshot = menu_snapshot();
    snapshot["settings"]["displayBrightness"] = json!(50);
    let frame = oled_frame(&snapshot);
    assert_eq!(
        pixel(&frame, 4, 30),
        rgb565(scale(rgb565_to_rgb(palette::GREEN_RGB565), 0.5))
    );
}

#[test]
fn sleeping_leds_are_deterministic_sparse_and_brightness_bounded() {
    let start = Instant::now();
    let mut first = SleepLedAnimation::with_seed(42);
    let mut second = SleepLedAnimation::with_seed(42);
    first.enter(start, 0.5, 0.5);
    second.enter(start, 0.5, 0.5);

    let first_frame = first.frames_at(start + Duration::from_secs(1));
    let second_frame = second.frames_at(start + Duration::from_secs(1));
    assert_eq!(first_frame.grid, second_frame.grid);
    assert_eq!(first_frame.keys, second_frame.keys);
    assert!(
        first_frame
            .grid
            .iter()
            .filter(|color| **color != [0; 3])
            .count()
            <= 4
    );
    assert!(
        first_frame
            .keys
            .iter()
            .filter(|color| **color != [0; 3])
            .count()
            <= 1
    );

    let maximum = scale([u8::MAX; 3], sleep_dim_brightness(0.5));
    for color in first_frame.grid.into_iter().chain(first_frame.keys) {
        assert!(color
            .iter()
            .zip(maximum)
            .all(|(value, limit)| *value <= limit));
    }

    let mut blackout = SleepLedAnimation::with_seed(42);
    blackout.enter(start, 0.0, 0.0);
    let blackout_frame = blackout.frames_at(start + Duration::from_secs(1));
    assert!(blackout_frame.grid.iter().all(|color| *color == [0; 3]));
    assert!(blackout_frame.keys.iter().all(|color| *color == [0; 3]));
}

#[test]
fn repeated_sleep_entries_do_not_restart_the_animation() {
    let start = Instant::now();
    let first_time = start + Duration::from_secs(1);
    let second_time = start + Duration::from_millis(1_200);

    let mut repeated = SleepLedAnimation::with_seed(7);
    repeated.enter(start, 1.0, 1.0);
    let _ = repeated.frames_at(first_time);
    repeated.enter(first_time, 1.0, 1.0);
    let repeated_frame = repeated.frames_at(second_time);

    let mut uninterrupted = SleepLedAnimation::with_seed(7);
    uninterrupted.enter(start, 1.0, 1.0);
    let _ = uninterrupted.frames_at(first_time);
    let uninterrupted_frame = uninterrupted.frames_at(second_time);

    assert_eq!(repeated_frame.grid, uninterrupted_frame.grid);
    assert_eq!(repeated_frame.keys, uninterrupted_frame.keys);
}

#[test]
fn waking_clears_sleep_animation_and_output_cache() {
    let mut cache = HardwareRenderCache {
        led_frame: Some([[1; 3]; 64]),
        neokey_colors: Some([[2; 3]; 4]),
        ..Default::default()
    };
    cache.sleep_leds.enter(Instant::now(), 1.0, 1.0);

    cache.clear_sleep_animation();

    assert!(!cache.sleep_leds.active());
    assert!(cache.led_frame.is_none());
    assert!(cache.neokey_colors.is_none());
}

#[test]
fn sleeping_animation_uses_display_off_instead_of_dim_timer_state() {
    let mut sleeping = snapshot_with_leds();
    sleeping["display"]["off"] = json!(true);
    sleeping["settings"]["ledsDimmed"] = json!(false);
    assert!(snapshot_display_off(&sleeping));

    sleeping["display"]["off"] = json!(false);
    sleeping["settings"]["ledsDimmed"] = json!(true);
    assert!(!snapshot_display_off(&sleeping));
}

#[test]
fn sleeping_animation_emits_at_entry_and_deadlines_only() {
    let (command_tx, command_rx) = mpsc::channel();
    let mut targets = HardwareRenderTargets {
        oled: OledSsd1351::new().unwrap(),
        seesaw_tx: command_tx,
        hdmi: None,
    };
    let sleeping = {
        let mut snapshot = snapshot_with_leds();
        snapshot["display"]["off"] = json!(true);
        snapshot
    };
    let mut cache = HardwareRenderCache::default();

    let deadline = render_snapshot_cached(&mut targets, &sleeping, &mut cache).unwrap();
    assert_eq!(command_rx.try_iter().count(), 2);

    let repeated_deadline = render_snapshot_cached(&mut targets, &sleeping, &mut cache).unwrap();
    assert_eq!(repeated_deadline, deadline);
    assert_eq!(command_rx.try_iter().count(), 0);

    let _ = cache.render_sleep_tick(&mut targets, deadline - Duration::from_millis(1));
    assert_eq!(command_rx.try_iter().count(), 0);
    let _ = cache.render_sleep_tick(&mut targets, deadline);
    assert!(command_rx.try_iter().count() > 0);
}

#[test]
fn sleeping_animation_restores_once_on_wake_and_stays_cached_awake() {
    let (command_tx, command_rx) = mpsc::channel();
    let mut targets = HardwareRenderTargets {
        oled: OledSsd1351::new().unwrap(),
        seesaw_tx: command_tx,
        hdmi: None,
    };
    let sleeping = {
        let mut snapshot = snapshot_with_leds();
        snapshot["display"]["off"] = json!(true);
        snapshot
    };
    let awake = snapshot_with_leds();
    let mut cache = HardwareRenderCache::default();

    render_snapshot_cached(&mut targets, &sleeping, &mut cache);
    let _ = command_rx.try_iter().count();
    render_snapshot_cached(&mut targets, &awake, &mut cache);
    let wake_commands = command_rx.try_iter().collect::<Vec<_>>();
    assert_eq!(wake_commands.len(), 2);
    assert!(wake_commands.iter().any(|command| {
        matches!(command, crate::seesaw_io::SeesawCommand::GridFrame(frame) if *frame == led_frame(&awake).unwrap())
    }));
    assert!(wake_commands.iter().any(|command| {
        matches!(command, crate::seesaw_io::SeesawCommand::NeoKeyColors(colors) if *colors == neokey_colors(&awake))
    }));

    render_snapshot_cached(&mut targets, &awake, &mut cache);
    assert_eq!(command_rx.try_iter().count(), 0);
}

#[test]
fn sleeping_animation_has_nonzero_rise_fall_expiry_and_tick_deadlines() {
    let start = Instant::now();
    let mut animation = SleepLedAnimation::with_seed(9);
    assert!(animation.enter(start, 1.0, 1.0));
    let entry = animation.frames_at(start);
    assert!(entry.grid.iter().all(|color| *color == [0; 3]));
    let first_key = animation.key_pulse_windows()[0];
    let next_tick = animation.next_deadline().unwrap();
    assert_eq!(next_tick, start + Duration::from_millis(100));
    assert!(animation
        .frames_if_due(start + Duration::from_millis(50))
        .is_none());

    let key = first_key.0;
    let rise = animation.frames_at(first_key.1 + first_key.2.duration_since(first_key.1) / 4);
    let peak = animation.frames_at(first_key.1 + first_key.2.duration_since(first_key.1) / 2);
    let fall = animation.frames_at(first_key.1 + first_key.2.duration_since(first_key.1) * 3 / 4);
    assert!(rise.keys[key].iter().any(|value| *value != 0));
    assert!(peak.keys[key].iter().any(|value| *value != 0));
    assert!(fall.keys[key].iter().any(|value| *value != 0));
    let level = |color: [u8; 3]| u16::from(color[0]) + u16::from(color[1]) + u16::from(color[2]);
    assert!(level(peak.keys[key]) > level(rise.keys[key]));
    assert!(level(peak.keys[key]) > level(fall.keys[key]));

    let expired = animation.frames_at(first_key.2);
    assert_eq!(expired.keys[key], [0; 3]);
}
