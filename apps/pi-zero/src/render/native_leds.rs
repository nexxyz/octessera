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

    #[test]
    fn grid_applies_brightness_and_sleep_dim() {
        let rgb = [255, 212, 71].repeat(64);
        let mut leds = NativeLedPresentation {
            grid: NativeGridPresentation {
                width: 8,
                height: 8,
                rgb,
                active: vec![true; 64],
            },
            brightness: 25,
            dimmed: false,
            off: false,
        };
        assert_eq!(native_grid_frame(&leds)[0], [64, 53, 18]);
        leds.dimmed = true;
        assert_eq!(native_grid_frame(&leds)[0], scale([255, 212, 71], 0.04));
        leds.brightness = 0;
        assert_eq!(native_grid_frame(&leds)[0], [0; 3]);
    }

    #[test]
    fn control_buttons_use_basis_point_brightness_and_dimming() {
        for brightness in [0_u8, 10, 75, 100] {
            for dimmed in [false, true] {
                let controls = NativeControlButtonPresentation {
                    colors: [[255, 128, 1]; 4],
                    brightness,
                };
                let brightness = u32::from(brightness);
                let basis_points = match (dimmed, brightness) {
                    (true, 0) => 0,
                    (true, _) => (brightness * 8).max(400),
                    (false, _) => brightness * 100,
                };
                let expected =
                    [255, 128, 1].map(|channel| ((channel * basis_points + 5_000) / 10_000) as u8);
                assert_eq!(native_control_colors(&controls, dimmed)[0], expected);
            }
        }
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
