use crate::seesaw_io::SeesawCommand;
use cellsymphony_hal::OledSsd1351;
use playback_runtime::RuntimeUiPulse;
use serde_json::Value;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

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
    pub seesaw_tx: &'a Sender<SeesawCommand>,
}

pub struct HardwareRenderCache {
    led_frame: Option<[[u8; 3]; 64]>,
    neokey_colors: Option<[[u8; 3]; 4]>,
    oled_signature: u64,
    oled_frame: Vec<u8>,
    event_dot_until: Option<Instant>,
    transport_flash_until: Option<Instant>,
    transport_flash: Option<String>,
}

impl HardwareRenderCache {
    pub fn new() -> Self {
        Self {
            led_frame: None,
            neokey_colors: None,
            oled_signature: 0,
            oled_frame: vec![0_u8; OLED_FRAME_BYTES],
            event_dot_until: None,
            transport_flash_until: None,
            transport_flash: None,
        }
    }

    pub fn apply_ui_pulse(&mut self, pulse: RuntimeUiPulse) {
        let now = Instant::now();
        match pulse {
            RuntimeUiPulse::TriggerPulse { duration_ms } => {
                self.event_dot_until = Some(now + Duration::from_millis(duration_ms));
            }
            RuntimeUiPulse::TransportFlash { flash, duration_ms } => {
                self.transport_flash = Some(flash);
                self.transport_flash_until = Some(now + Duration::from_millis(duration_ms));
            }
        }
    }

    pub fn snapshot_with_transients(&mut self, snapshot: &Value) -> Value {
        let now = Instant::now();
        let event_active = self.event_dot_until.is_some_and(|until| now < until);
        let transport_active = self.transport_flash_until.is_some_and(|until| now < until);
        if !event_active {
            self.event_dot_until = None;
        }
        if !transport_active {
            self.transport_flash_until = None;
            self.transport_flash = None;
        }
        if !event_active && !transport_active {
            return snapshot.clone();
        }
        let mut snapshot = snapshot.clone();
        if event_active {
            snapshot["eventDotOn"] = serde_json::json!(true);
        }
        if transport_active {
            if let Some(flash) = &self.transport_flash {
                snapshot["transportFlash"] = serde_json::json!(flash);
            }
        }
        snapshot
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
        if cache.led_frame.as_ref() != Some(&frame)
            && targets
                .seesaw_tx
                .send(SeesawCommand::GridFrame(frame))
                .is_ok()
        {
            cache.led_frame = Some(frame);
        }
    }

    let neokey = neokey_colors(snapshot);
    let previous_neokey = cache.neokey_colors.unwrap_or([[u8::MAX; 3]; 4]);
    if previous_neokey != neokey
        && targets
            .seesaw_tx
            .send(SeesawCommand::NeoKeyColors(neokey))
            .is_ok()
    {
        cache.neokey_colors = Some(neokey);
    }

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
            let sent = targets
                .seesaw_tx
                .send(SeesawCommand::GridFrame(frame))
                .is_ok();
            if let Some(metrics) = metrics.as_deref_mut() {
                metrics.led_write = write_started.elapsed();
            }
            if sent {
                cache.led_frame = Some(frame);
            }
        }
    }

    let neokey_started = Instant::now();
    let neokey = neokey_colors(snapshot);
    if let Some(metrics) = metrics.as_deref_mut() {
        metrics.neokey_build = neokey_started.elapsed();
    }
    let previous_neokey = cache.neokey_colors.unwrap_or([[u8::MAX; 3]; 4]);
    let neokey_write_started = Instant::now();
    let mut changed = false;
    for (index, color) in neokey.iter().enumerate() {
        if previous_neokey.get(index) == Some(color) {
            continue;
        }
        changed = true;
    }
    let sent = !changed
        || targets
            .seesaw_tx
            .send(SeesawCommand::NeoKeyColors(neokey))
            .is_ok();
    if let Some(metrics) = metrics.as_deref_mut() {
        metrics.neokey_write = neokey_write_started.elapsed();
    }
    if sent {
        cache.neokey_colors = Some(neokey);
    }

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
    let settings = snapshot.get("settings").unwrap_or(&Value::Null);
    let display = snapshot.get("display").unwrap_or(&Value::Null);
    let mut brightness = brightness_scale(settings.get("gridBrightness"));
    if display.get("off").and_then(Value::as_bool).unwrap_or(false) {
        brightness *= 0.08;
    }
    let Some(rgb) = snapshot.get("leds")?.get("rgb").and_then(Value::as_array) else {
        return legacy_led_frame(snapshot, brightness);
    };
    let mut frame = [[0_u8; 3]; 64];
    for (idx, cell) in frame.iter_mut().enumerate() {
        let offset = idx * 3;
        *cell = scale(
            [
                scaled_u8(rgb.get(offset)),
                scaled_u8(rgb.get(offset + 1)),
                scaled_u8(rgb.get(offset + 2)),
            ],
            brightness,
        );
    }
    Some(frame)
}

fn legacy_led_frame(snapshot: &Value, brightness: f32) -> Option<[[u8; 3]; 64]> {
    let cells = snapshot.get("leds")?.get("cells")?.as_array()?;
    let mut frame = [[0_u8; 3]; 64];
    for (idx, cell) in cells.iter().take(64).enumerate() {
        frame[idx] = scale(
            [
                scaled_u8(cell.get("r")),
                scaled_u8(cell.get("g")),
                scaled_u8(cell.get("b")),
            ],
            brightness,
        );
    }
    Some(frame)
}

pub fn neokey_colors(snapshot: &Value) -> [[u8; 3]; 4] {
    let settings = snapshot.get("settings").unwrap_or(&Value::Null);
    let display = snapshot.get("display").unwrap_or(&Value::Null);
    let mut button_scale = brightness_scale(settings.get("buttonBrightness"));
    if display.get("off").and_then(Value::as_bool).unwrap_or(false) {
        button_scale *= 0.08;
    }
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
    let flash = snapshot
        .get("transportFlash")
        .or_else(|| settings.get("transportFlash"))
        .and_then(Value::as_str)
        .unwrap_or("none");
    let icon = snapshot
        .get("transportIcon")
        .and_then(Value::as_str)
        .unwrap_or("stop");
    let back = scale([90, 0, 0], button_scale);
    let space = if icon == "stop" {
        scale([255, 51, 51], button_scale)
    } else if icon == "pause" {
        scale([215, 255, 232], button_scale)
    } else if flash == "measure" {
        scale([255, 160, 0], button_scale)
    } else if flash == "beat" {
        scale([51, 255, 102], button_scale)
    } else {
        scale([0, 80, 0], button_scale)
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
    let off = snapshot_display_off(snapshot);
    if !off {
        let _ = oled.display_on();
    }
    oled_frame_into(snapshot, frame);
    let _ = oled.write_frame(frame);
    if off {
        let _ = oled.display_off();
    }
}

pub fn render_boot_splash(oled: &mut OledSsd1351) {
    let _ = oled.display_on();
    let snapshot = serde_json::json!({
        "display": {
            "off": false,
            "splash": "startup",
            "toast": ""
        },
        "settings": { "displayBrightness": 100 }
    });
    let mut frame = vec![0_u8; OLED_FRAME_BYTES];
    render_oled(oled, &snapshot, &mut frame);
}

pub fn render_shutdown_splash(oled: &mut OledSsd1351) {
    let _ = oled.display_on();
    let snapshot = serde_json::json!({
        "display": {
            "off": false,
            "splash": "shutdown",
            "toast": ""
        },
        "settings": { "displayBrightness": 100 }
    });
    let mut frame = vec![0_u8; OLED_FRAME_BYTES];
    render_oled(oled, &snapshot, &mut frame);
}

fn render_oled_profiled(
    oled: &mut OledSsd1351,
    snapshot: &Value,
    frame: &mut [u8],
    metrics: Option<&mut RenderProfileMetrics>,
) {
    let off = snapshot_display_off(snapshot);
    if !off {
        let _ = oled.display_on();
    }
    let build_started = Instant::now();
    oled_frame_into(snapshot, frame);
    let build_duration = build_started.elapsed();
    let write_started = Instant::now();
    let _ = oled.write_frame(frame);
    if off {
        let _ = oled.display_off();
    }
    if let Some(metrics) = metrics {
        metrics.oled_frame_build = build_duration;
        metrics.oled_write = write_started.elapsed();
        metrics.oled_rendered = true;
    }
}

fn snapshot_display_off(snapshot: &Value) -> bool {
    snapshot
        .get("display")
        .and_then(|display| display.get("off"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
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
