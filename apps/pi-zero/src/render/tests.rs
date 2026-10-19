use super::oled_error_tests::{menu_snapshot, pixel};
use super::*;
use platform_core::palette;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

fn rgb565_to_rgb(value: u16) -> [u8; 3] {
    [
        ((((value >> 11) & 0x1f) * 255) / 31) as u8,
        ((((value >> 5) & 0x3f) * 255) / 63) as u8,
        (((value & 0x1f) * 255) / 31) as u8,
    ]
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
    assert_eq!(oled::reference::title_text_for_oled("B"), "/Build");
    assert_eq!(oled::reference::title_text_for_oled("/B"), "/Build");
    assert_eq!(
        oled::reference::title_text_for_oled("/B/L1: life"),
        "/B/L1: life"
    );
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

#[cfg(not(any(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "hardware-orange-pi-zero-2w"
)))]
mod hardware_render_tests {
    use super::*;
    use crate::render_loop::test_scenes::test_scene;
    use playback_runtime::oled_frame::OledPresentationMetrics;
    use std::sync::mpsc;

    fn presentation(off: bool) -> LatestPresentation {
        let mut hardware =
            test_scene().into_hardware_presentation(OledPresentationMetrics::default(), None);
        hardware.leds.grid.rgb = palette::YELLOW.repeat(64);
        hardware.leds.brightness = 50;
        hardware.leds.off = off;
        hardware.neo_key.colors = [
            palette::MAGENTA,
            palette::MAGENTA,
            dim(palette::GRAY, 3),
            dim(palette::GRAY, 3),
        ];
        hardware.neo_key.brightness = 100;
        let pixels = playback_runtime::oled_frame::render_oled_frame(&hardware.oled);
        LatestPresentation {
            hardware: Box::new(hardware),
            oled: OledFramePublication::test_native(1, pixels),
            completion: None,
        }
    }

    fn fake_targets() -> (
        HardwareRenderTargets,
        mpsc::Receiver<crate::seesaw_io::SeesawCommand>,
    ) {
        let (seesaw_tx, seesaw_rx) = mpsc::channel();
        let targets = HardwareRenderTargets {
            oled: crate::render::test_oled_output::fake_oled_output(),
            seesaw_tx,
            oled_handoff: None,
            hdmi: hdmi::HdmiFramebuffer::new(),
        };
        (targets, seesaw_rx)
    }

    #[test]
    fn hdmi_failure_does_not_block_oled_publication() {
        let (mut targets, _seesaw_rx) = fake_targets();
        let mut cache = HardwareRenderCache::default();
        let mut live = presentation(false);
        live.hardware.hdmi.mode = playback_runtime::NativeHdmiMode::LiveGrid;
        let now = Instant::now();

        let _ = live.render_oled_leds(&mut targets, &mut cache, now);
        let _ = live.render_hdmi(&mut targets, &mut cache, now);

        assert_eq!(cache.oled_render_count(), 1);
        assert_eq!(cache.hdmi_signature, 0);
    }

    #[test]
    fn sleeping_animation_emits_at_entry_and_deadlines_only() {
        let (mut targets, command_rx) = fake_targets();
        let sleeping = presentation(true);
        let mut cache = HardwareRenderCache::default();
        let now = Instant::now();

        let deadline = sleeping.render_leds(&mut targets, &mut cache, now).unwrap();
        assert_eq!(command_rx.try_iter().count(), 2);

        let repeated_deadline = sleeping.render_leds(&mut targets, &mut cache, now).unwrap();
        assert_eq!(repeated_deadline, deadline);
        assert_eq!(command_rx.try_iter().count(), 0);

        let _ = cache.render_sleep_tick(&mut targets, deadline - Duration::from_millis(1));
        assert_eq!(command_rx.try_iter().count(), 0);
        let _ = cache.render_sleep_tick(&mut targets, deadline);
        assert!(command_rx.try_iter().count() > 0);
    }

    #[test]
    fn sleeping_animation_restores_once_on_wake_and_stays_cached_awake() {
        let (mut targets, command_rx) = fake_targets();
        let sleeping = presentation(true);
        let awake = presentation(false);
        let mut cache = HardwareRenderCache::default();
        let now = Instant::now();

        sleeping.render_leds(&mut targets, &mut cache, now);
        let _ = command_rx.try_iter().count();
        awake.render_leds(&mut targets, &mut cache, now);
        let wake_commands = command_rx.try_iter().collect::<Vec<_>>();
        assert_eq!(wake_commands.len(), 2);
        let expected_grid = native_leds::native_grid_frame(&awake.hardware.leds);
        let expected_keys = native_leds::native_control_colors(&awake.hardware.neo_key, false);
        assert!(wake_commands.iter().any(|command| matches!(
            command,
            crate::seesaw_io::SeesawCommand::GridFrame(frame) if *frame == expected_grid
        )));
        assert!(wake_commands.iter().any(|command| matches!(
            command,
            crate::seesaw_io::SeesawCommand::NeoKeyColors(colors) if *colors == expected_keys
        )));

        awake.render_leds(&mut targets, &mut cache, now);
        assert_eq!(command_rx.try_iter().count(), 0);
    }
}
