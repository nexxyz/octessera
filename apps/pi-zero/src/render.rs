use cellsymphony_hal::{NeoKey, NeoTrellis, OledSsd1351};
use serde_json::Value;
use std::time::Instant;

mod oled;
mod profile;

pub(crate) use oled::OLED_FRAME_BYTES;
#[cfg(test)]
use oled::{glyph_rows, oled_frame};
use oled::{oled_frame_into, oled_signature};
pub use profile::RenderProfileMetrics;

const SPLASH_REGULAR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/splash_regular.rgb565"));
const SPLASH_SEPIA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/splash_sepia.rgb565"));

pub struct HardwareRenderTargets<'a> {
    pub oled: &'a mut OledSsd1351,
    pub trellis: &'a mut NeoTrellis,
    pub neokey: &'a mut NeoKey,
}

pub struct HardwareRenderCache {
    led_frame: Option<[[u8; 3]; 64]>,
    neokey_colors: Option<[[u8; 3]; 4]>,
    oled_signature: u64,
    oled_frame: Vec<u8>,
}

impl HardwareRenderCache {
    pub fn new() -> Self {
        Self {
            led_frame: None,
            neokey_colors: None,
            oled_signature: 0,
            oled_frame: vec![0_u8; OLED_FRAME_BYTES],
        }
    }
}

impl Default for HardwareRenderCache {
    fn default() -> Self {
        Self::new()
    }
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
        render_oled(targets.oled, snapshot, &mut cache.oled_frame);
    }
}

pub fn render_snapshot_cached_profiled(
    targets: &mut HardwareRenderTargets<'_>,
    snapshot: &Value,
    cache: &mut HardwareRenderCache,
    mut metrics: Option<&mut RenderProfileMetrics>,
) {
    let started = Instant::now();
    if let Some(frame) = led_frame(snapshot) {
        if let Some(metrics) = metrics.as_deref_mut() {
            metrics.led_extract = started.elapsed();
        }
        if cache.led_frame.as_ref() != Some(&frame) {
            let write_started = Instant::now();
            let _ = targets.trellis.write_led_frame(&frame);
            if let Some(metrics) = metrics.as_deref_mut() {
                metrics.led_write = write_started.elapsed();
            }
            cache.led_frame = Some(frame);
        }
    }

    let neokey_started = Instant::now();
    let neokey = neokey_colors(snapshot);
    if let Some(metrics) = metrics.as_deref_mut() {
        metrics.neokey_build = neokey_started.elapsed();
    }
    let previous_neokey = cache.neokey_colors.unwrap_or([[u8::MAX; 3]; 4]);
    let neokey_write_started = Instant::now();
    for (index, color) in neokey.iter().enumerate() {
        if previous_neokey.get(index) == Some(color) {
            continue;
        }
        let _ = targets
            .neokey
            .set_led(index as u8, color[0], color[1], color[2]);
    }
    if let Some(metrics) = metrics.as_deref_mut() {
        metrics.neokey_write = neokey_write_started.elapsed();
    }
    cache.neokey_colors = Some(neokey);

    let signature_started = Instant::now();
    let signature = oled_signature(snapshot);
    if let Some(metrics) = metrics.as_deref_mut() {
        metrics.oled_signature = signature_started.elapsed();
    }
    if cache.oled_signature != signature {
        cache.oled_signature = signature;
        render_oled_profiled(targets.oled, snapshot, &mut cache.oled_frame, metrics);
    }
}

pub fn led_frame(snapshot: &Value) -> Option<[[u8; 3]; 64]> {
    let Some(rgb) = snapshot.get("leds")?.get("rgb").and_then(Value::as_array) else {
        return legacy_led_frame(snapshot);
    };
    let mut frame = [[0_u8; 3]; 64];
    for (idx, cell) in frame.iter_mut().enumerate() {
        let offset = idx * 3;
        *cell = [
            scaled_u8(rgb.get(offset)),
            scaled_u8(rgb.get(offset + 1)),
            scaled_u8(rgb.get(offset + 2)),
        ];
    }
    Some(frame)
}

fn legacy_led_frame(snapshot: &Value) -> Option<[[u8; 3]; 64]> {
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

fn render_oled(oled: &mut OledSsd1351, snapshot: &Value, frame: &mut [u8]) {
    oled_frame_into(snapshot, frame);
    let _ = oled.write_frame(frame);
}

fn render_oled_profiled(
    oled: &mut OledSsd1351,
    snapshot: &Value,
    frame: &mut [u8],
    metrics: Option<&mut RenderProfileMetrics>,
) {
    let build_started = Instant::now();
    oled_frame_into(snapshot, frame);
    let build_duration = build_started.elapsed();
    let write_started = Instant::now();
    let _ = oled.write_frame(frame);
    if let Some(metrics) = metrics {
        metrics.oled_frame_build = build_duration;
        metrics.oled_write = write_started.elapsed();
        metrics.oled_rendered = true;
    }
}

pub(crate) fn fault_oled_frame_into(lines: &[String], frame: &mut [u8], lit: bool) {
    oled::fault_frame_into(lines, frame, lit);
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

#[rustfmt::skip]
pub(super) fn scale(rgb: [u8; 3], factor: f32) -> [u8; 3] { [
    ((rgb[0] as f32) * factor).round().clamp(0.0, 255.0) as u8,
    ((rgb[1] as f32) * factor).round().clamp(0.0, 255.0) as u8,
    ((rgb[2] as f32) * factor).round().clamp(0.0, 255.0) as u8,
] }

pub(super) fn rgb565(rgb: [u8; 3]) -> u16 {
    ((u16::from(rgb[0]) & 0xF8) << 8) | ((u16::from(rgb[1]) & 0xFC) << 3) | (u16::from(rgb[2]) >> 3)
}

#[cfg(test)]
mod tests;
