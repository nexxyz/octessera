use super::{scale, sleep_dim_brightness, HardwareRenderCache, HardwareRenderTargets};
use playback_runtime::{
    NativeControlButtonPresentation, NativeHardwarePresentation, NativeLedPresentation,
};
use std::time::Instant;

fn brightness_scale(value: u8) -> f32 {
    f32::from(value.min(100)) / 100.0
}

pub(crate) fn native_grid_frame(leds: &NativeLedPresentation) -> [[u8; 3]; 64] {
    assert_native_grid(leds);
    let mut brightness = brightness_scale(leds.brightness);
    if leds.dimmed {
        brightness = sleep_dim_brightness(brightness);
    }
    std::array::from_fn(|index| {
        let offset = index * 3;
        scale(
            [
                leds.grid.rgb[offset],
                leds.grid.rgb[offset + 1],
                leds.grid.rgb[offset + 2],
            ],
            brightness,
        )
    })
}

fn assert_native_grid(leds: &NativeLedPresentation) {
    assert_eq!((leds.grid.width, leds.grid.height), (8, 8));
    assert_eq!(leds.grid.rgb.len(), 64 * 3);
    assert_eq!(leds.grid.active.len(), 64);
}

pub(crate) fn native_control_colors(
    controls: &NativeControlButtonPresentation,
    dimmed: bool,
) -> [[u8; 3]; 4] {
    scale_control_colors(controls.colors, controls.brightness, dimmed)
}

fn scale_control_colors(colors: [[u8; 3]; 4], brightness: u8, dimmed: bool) -> [[u8; 3]; 4] {
    let brightness = u32::from(brightness.min(100));
    let basis_points = if dimmed {
        if brightness == 0 {
            0
        } else {
            (brightness * 8).max(400)
        }
    } else {
        brightness * 100
    };
    colors.map(|rgb| {
        rgb.map(|channel| ((u32::from(channel) * basis_points + 5_000) / 10_000).min(255) as u8)
    })
}

pub(crate) fn render_native_leds_at(
    targets: &mut HardwareRenderTargets,
    presentation: &NativeHardwarePresentation,
    cache: &mut HardwareRenderCache,
    now: Instant,
) -> Option<Instant> {
    assert_native_grid(&presentation.leds);
    if presentation.leds.off {
        let entered = cache.sleep_leds.enter(
            now,
            brightness_scale(presentation.leds.brightness),
            brightness_scale(presentation.neo_key.brightness),
        );
        if entered {
            let frames = cache.sleep_leds.frames_at(now);
            super::send_sleep_led_frames(targets, cache, frames);
        } else if let Some(frames) = cache.sleep_leds.frames_if_due(now) {
            super::send_sleep_led_frames(targets, cache, frames);
        }
        cache.sleep_leds.next_deadline()
    } else {
        if cache.sleep_leds.active() {
            cache.clear_sleep_animation();
        }
        super::send_grid_frame(targets, cache, native_grid_frame(&presentation.leds));
        super::send_neokey_colors(
            targets,
            cache,
            native_control_colors(&presentation.neo_key, presentation.leds.dimmed),
        );
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use playback_runtime::{
        NativeControlButtonPresentation, NativeGridPresentation, NativeLedPresentation,
    };
    use serde_json::json;
    use std::time::Instant;

    #[test]
    fn typed_grid_and_buttons_match_legacy_brightness_and_dimmed_pixels() {
        let rgb = [255, 212, 71].repeat(64);
        let mut leds = NativeLedPresentation {
            grid: NativeGridPresentation {
                width: 8,
                height: 8,
                rgb: rgb.clone(),
                active: vec![true; 64],
            },
            brightness: 25,
            dimmed: false,
            off: false,
        };
        let controls = NativeControlButtonPresentation {
            colors: [[221, 130, 205], [255, 0, 0], [67, 68, 71], [67, 68, 71]],
            brightness: 35,
        };
        for dimmed in [false, true] {
            leds.dimmed = dimmed;
            let snapshot = json!({
                "display": {"off": false},
                "settings": {"gridBrightness": 25, "buttonBrightness": 35, "ledsDimmed": dimmed},
                "leds": {"rgb": rgb},
                "neoKeyLeds": {"back": controls.colors[0], "space": controls.colors[1],
                    "shift": controls.colors[2], "fn": controls.colors[3]}
            });
            assert_eq!(
                native_grid_frame(&leds),
                super::super::led_frame(&snapshot).unwrap()
            );
            assert_eq!(
                native_control_colors(&controls, dimmed),
                super::super::neokey_colors(&snapshot)
            );
        }
        assert_eq!(native_grid_frame(&leds)[0], [10, 8, 3]);
    }

    #[test]
    fn typed_sleep_uses_same_seed_and_animation_brightness_as_legacy() {
        let start = Instant::now();
        let mut typed = super::super::SleepLedAnimation::with_seed(77);
        let mut legacy = super::super::SleepLedAnimation::with_seed(77);
        let leds = NativeLedPresentation {
            grid: NativeGridPresentation {
                width: 8,
                height: 8,
                rgb: vec![0; 192],
                active: vec![false; 64],
            },
            brightness: 25,
            dimmed: false,
            off: true,
        };
        let buttons = NativeControlButtonPresentation {
            colors: [[0; 3]; 4],
            brightness: 35,
        };
        let snapshot = json!({"display": {"off": true}, "settings": {"gridBrightness": 25, "buttonBrightness": 35}});
        assert!(typed.enter(
            start,
            brightness_scale(leds.brightness),
            brightness_scale(buttons.brightness)
        ));
        assert!(legacy.enter(
            start,
            super::super::brightness_scale(Some(&snapshot["settings"]["gridBrightness"])),
            super::super::brightness_scale(Some(&snapshot["settings"]["buttonBrightness"]))
        ));
        let _ = typed.frames_at(start);
        let _ = legacy.frames_at(start);
        let later = start + std::time::Duration::from_millis(100);
        let typed_frame = typed.frames_at(later);
        let legacy_frame = legacy.frames_at(later);
        assert_eq!(typed_frame.grid, legacy_frame.grid);
        assert_eq!(typed_frame.keys, legacy_frame.keys);
        assert!(typed_frame.grid.iter().any(|rgb| *rgb != [0; 3]));
        assert_eq!(typed.next_deadline(), legacy.next_deadline());
    }

    #[test]
    #[should_panic(expected = "left: 191")]
    fn invalid_native_grid_length_fails_closed() {
        native_grid_frame(&NativeLedPresentation {
            grid: NativeGridPresentation {
                width: 8,
                height: 8,
                rgb: vec![0; 191],
                active: vec![false; 64],
            },
            brightness: 25,
            dimmed: false,
            off: false,
        });
    }
}
