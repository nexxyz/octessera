use cellsymphony_hal::{NeoKey, NeoTrellis, OledSsd1351};
use serde_json::Value;

pub struct HardwareRenderTargets<'a> {
    pub oled: &'a mut OledSsd1351,
    pub trellis: &'a mut NeoTrellis,
    pub neokey: &'a mut NeoKey,
}

#[derive(Default)]
pub struct HardwareRenderCache {
    led_frame: Option<[[u8; 3]; 64]>,
    neokey_colors: Option<[[u8; 3]; 4]>,
    oled_signature: u64,
}

pub fn render_snapshot_cached(
    targets: &mut HardwareRenderTargets<'_>,
    snapshot: &Value,
    cache: &mut HardwareRenderCache,
) {
    if let Some(frame) = led_frame(snapshot) {
        if cache.led_frame.as_ref() != Some(&frame) {
            let _ = targets.trellis.write_led_frame(&frame);
            cache.led_frame = Some(frame);
        }
    }

    let neokey = neokey_colors(snapshot);
    let previous_neokey = cache.neokey_colors.unwrap_or([[u8::MAX; 3]; 4]);
    for (index, color) in neokey.iter().enumerate() {
        if previous_neokey.get(index) == Some(color) {
            continue;
        }
        let _ = targets
            .neokey
            .set_led(index as u8, color[0], color[1], color[2]);
    }
    cache.neokey_colors = Some(neokey);

    let signature = oled_signature(snapshot);
    if cache.oled_signature != signature {
        cache.oled_signature = signature;
        render_oled(targets.oled, snapshot);
    }
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
    let frame = oled_frame(snapshot);
    let _ = oled.write_frame(&frame);
}

fn oled_signature(snapshot: &Value) -> u64 {
    let settings = snapshot.get("settings").unwrap_or(&Value::Null);
    let display = snapshot.get("display").unwrap_or(&Value::Null);
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash_value(&mut hash, settings.get("displayBrightness"));
    hash_value(&mut hash, display.get("off"));
    hash_value(&mut hash, display.get("splash"));
    hash_value(&mut hash, display.get("title"));
    hash_value(&mut hash, display.get("lines"));
    hash_value(&mut hash, display.get("editing"));
    hash
}

fn hash_value(hash: &mut u64, value: Option<&Value>) {
    match value.unwrap_or(&Value::Null) {
        Value::Null => mix_hash(hash, 0),
        Value::Bool(value) => mix_hash(hash, u64::from(*value)),
        Value::Number(value) => mix_hash(hash, value.as_u64().unwrap_or(0)),
        Value::String(value) => {
            for byte in value.as_bytes() {
                mix_hash(hash, u64::from(*byte));
            }
        }
        Value::Array(values) => {
            for value in values {
                hash_value(hash, Some(value));
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                for byte in key.as_bytes() {
                    mix_hash(hash, u64::from(*byte));
                }
                hash_value(hash, Some(value));
            }
        }
    }
}

fn mix_hash(hash: &mut u64, value: u64) {
    *hash ^= value;
    *hash = hash.wrapping_mul(0x100_0000_01b3);
}

fn oled_frame(snapshot: &Value) -> Vec<u8> {
    let settings = snapshot.get("settings").unwrap_or(&Value::Null);
    let display = snapshot.get("display").unwrap_or(&Value::Null);
    let brightness = brightness_scale(settings.get("displayBrightness"));
    let mut frame = vec![0_u8; 128 * 128 * 2];
    if display.get("off").and_then(Value::as_bool).unwrap_or(false) {
        return frame;
    }
    if let Some(splash) = display.get("splash").and_then(Value::as_str) {
        if !splash.is_empty() {
            render_splash_frame(&mut frame, splash, brightness);
            return frame;
        }
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
    frame
}

fn render_splash_frame(frame: &mut [u8], splash: &str, brightness: f32) {
    let accent = match splash {
        "Starting up" => scale([70, 125, 190], brightness),
        "Going to sleep" => scale([160, 108, 42], brightness),
        _ => scale([95, 155, 120], brightness),
    };
    fill_rect(frame, 0, 0, 128, 128, rgb565(scale([3, 7, 10], brightness)));
    fill_rect(frame, 0, 0, 128, 10, rgb565(accent));
    fill_rect(frame, 0, 118, 128, 10, rgb565(accent));
    fill_rect(frame, 12, 18, 104, 92, rgb565(scale([10, 16, 20], brightness)));

    let lines = match splash {
        "Starting up" => ["STARTING", "UP"].as_slice(),
        "Going to sleep" => ["GOING TO", "SLEEP"].as_slice(),
        "Waking up" => ["WAKING", "UP"].as_slice(),
        _ => ["CELL", "SYMPHONY"].as_slice(),
    };
    let line_height = 24_i32;
    let total_height = (lines.len() as i32 * line_height) - 6;
    let start_y = ((128 - total_height) / 2).max(16);
    for (index, line) in lines.iter().enumerate() {
        let width = text_width(line, 3);
        let x = ((128_i32 - width) / 2).max(8);
        let y = start_y + (index as i32 * line_height);
        draw_text(frame, line, x, y, 3, rgb565(scale([240, 244, 228], brightness)));
    }
}

fn fill_rect(frame: &mut [u8], x: usize, y: usize, width: usize, height: usize, color: u16) {
    for py in y..(y + height).min(128) {
        for px in x..(x + width).min(128) {
            let idx = (py * 128 + px) * 2;
            frame[idx] = (color >> 8) as u8;
            frame[idx + 1] = color as u8;
        }
    }
}

fn draw_text(frame: &mut [u8], text: &str, x: i32, y: i32, scale: usize, color: u16) {
    let mut cursor_x = x;
    for ch in text.chars() {
        if ch == ' ' {
            cursor_x += (6 * scale) as i32;
            continue;
        }
        for (row, bits) in glyph_rows(ch).iter().enumerate() {
            for col in 0..5 {
                if (bits >> (4 - col)) & 1 == 0 {
                    continue;
                }
                fill_rect(
                    frame,
                    (cursor_x + (col * scale) as i32).max(0) as usize,
                    (y + (row * scale) as i32).max(0) as usize,
                    scale,
                    scale,
                    color,
                );
            }
        }
        cursor_x += (6 * scale) as i32;
    }
}

fn text_width(text: &str, scale: usize) -> i32 {
    let chars = text.chars().count() as i32;
    if chars == 0 {
        0
    } else {
        chars * (6 * scale) as i32 - scale as i32
    }
}

fn glyph_rows(ch: char) -> [u8; 7] {
    match ch {
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'G' => [0x0F, 0x10, 0x10, 0x13, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x1F],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        _ => [0; 7],
    }
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

    #[test]
    fn oled_frame_renders_splash_text() {
        let snapshot = json!({
            "display": { "off": false, "splash": "Starting up" },
            "settings": { "displayBrightness": 100 }
        });

        let frame = oled_frame(&snapshot);
        assert!(frame.iter().any(|byte| *byte != 0));
    }
}
