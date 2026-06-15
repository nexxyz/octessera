use cellsymphony_hal::{NeoKey, NeoTrellis, OledSsd1351};
use serde_json::Value;

pub struct HardwareRenderTargets<'a> {
    pub oled: &'a mut OledSsd1351,
    pub trellis: &'a mut NeoTrellis,
    pub neokey: &'a mut NeoKey,
}

pub fn render_snapshot(targets: &mut HardwareRenderTargets<'_>, snapshot: &Value) {
    if let Some(frame) = led_frame(snapshot) {
        let _ = targets.trellis.write_led_frame(&frame);
    }
    render_neokey(targets.neokey, snapshot);
    render_oled(targets.oled, snapshot);
}

pub fn led_frame(snapshot: &Value) -> Option<[[u8; 3]; 64]> {
    let cells = snapshot.get("leds")?.get("cells")?.as_array()?;
    let mut frame = [[0_u8; 3]; 64];
    for (idx, cell) in cells.iter().take(64).enumerate() {
        frame[idx] = [
            scaled_u8(cell.get("r")),
            scaled_u8(cell.get("g")),
            scaled_u8(cell.get("b")),
        ];
    }
    Some(frame)
}

fn render_neokey(neokey: &mut NeoKey, snapshot: &Value) {
    let [back, space, shift, func] = neokey_colors(snapshot);
    let _ = neokey.set_led(0, back[0], back[1], back[2]);
    let _ = neokey.set_led(1, space[0], space[1], space[2]);
    let _ = neokey.set_led(2, shift[0], shift[1], shift[2]);
    let _ = neokey.set_led(3, func[0], func[1], func[2]);
}

pub fn neokey_colors(snapshot: &Value) -> [[u8; 3]; 4] {
    let settings = snapshot.get("settings").unwrap_or(&Value::Null);
    let display = snapshot.get("display").unwrap_or(&Value::Null);
    let transport = snapshot.get("transport").unwrap_or(&Value::Null);
    let button_scale = brightness_scale(settings.get("buttonBrightness"));
    let combined = settings
        .get("combinedModifierHeld")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let shift_held = settings
        .get("shiftHeld")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let fn_held = settings
        .get("fnHeld")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let flash = settings
        .get("transportFlash")
        .and_then(Value::as_str)
        .unwrap_or("none");
    let playing = transport
        .get("playing")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let off = display.get("off").and_then(Value::as_bool).unwrap_or(false);
    let back = if off {
        [0, 0, 0]
    } else {
        scale([90, 0, 0], button_scale)
    };
    let space = if !playing {
        [0, 0, 0]
    } else if flash == "measure" {
        scale([80, 80, 255], button_scale)
    } else if flash == "beat" {
        scale([40, 40, 120], button_scale)
    } else {
        scale([0, 35, 0], button_scale)
    };
    let shift = if combined {
        scale([0, 0, 180], button_scale)
    } else if shift_held {
        scale([180, 140, 0], button_scale)
    } else {
        [0, 0, 0]
    };
    let func = if combined {
        scale([0, 0, 180], button_scale)
    } else if fn_held {
        scale([180, 140, 0], button_scale)
    } else {
        [0, 0, 0]
    };
    [back, space, shift, func]
}

fn render_oled(oled: &mut OledSsd1351, snapshot: &Value) {
    let settings = snapshot.get("settings").unwrap_or(&Value::Null);
    let display = snapshot.get("display").unwrap_or(&Value::Null);
    let brightness = brightness_scale(settings.get("displayBrightness"));
    let mut frame = vec![0_u8; 128 * 128 * 2];
    if display.get("off").and_then(Value::as_bool).unwrap_or(false) {
        let _ = oled.write_frame(&frame);
        return;
    }
    let color = rgb565(scale([40, 80, 120], brightness));
    for y in 0..128_usize {
        for x in 0..128_usize {
            if y < 8 || (x + y) % 31 == 0 {
                let idx = (y * 128 + x) * 2;
                frame[idx] = (color >> 8) as u8;
                frame[idx + 1] = color as u8;
            }
        }
    }
    let _ = oled.write_frame(&frame);
}

fn scaled_u8(value: Option<&Value>) -> u8 {
    value.and_then(Value::as_u64).unwrap_or(0).min(255) as u8
}

fn brightness_scale(value: Option<&Value>) -> f32 {
    value
        .and_then(Value::as_u64)
        .map(|value| value.min(100) as f32 / 100.0)
        .unwrap_or(1.0)
}

fn scale(rgb: [u8; 3], factor: f32) -> [u8; 3] {
    [
        ((rgb[0] as f32) * factor).round().clamp(0.0, 255.0) as u8,
        ((rgb[1] as f32) * factor).round().clamp(0.0, 255.0) as u8,
        ((rgb[2] as f32) * factor).round().clamp(0.0, 255.0) as u8,
    ]
}

fn rgb565(rgb: [u8; 3]) -> u16 {
    ((u16::from(rgb[0]) & 0xF8) << 8) | ((u16::from(rgb[1]) & 0xFC) << 3) | (u16::from(rgb[2]) >> 3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn led_frame_clamps_native_snapshot_values() {
        let snapshot = json!({
            "leds": { "cells": [{ "r": 300, "g": 7, "b": 8 }] }
        });
        let frame = led_frame(&snapshot).unwrap();
        assert_eq!(frame[0], [255, 7, 8]);
    }

    #[test]
    fn neokey_colors_cover_shift_fn_and_combined_modifier() {
        let shifted = json!({
            "display": { "off": false },
            "transport": { "playing": false },
            "settings": { "buttonBrightness": 100, "shiftHeld": true, "fnHeld": false, "combinedModifierHeld": false }
        });
        let combined = json!({
            "display": { "off": false },
            "transport": { "playing": false },
            "settings": { "buttonBrightness": 100, "shiftHeld": false, "fnHeld": false, "combinedModifierHeld": true }
        });

        assert_eq!(neokey_colors(&shifted)[2], [180, 140, 0]);
        assert_eq!(neokey_colors(&combined)[2], [0, 0, 180]);
        assert_eq!(neokey_colors(&combined)[3], [0, 0, 180]);
    }
}
