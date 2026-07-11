use platform_core::palette;
use serde_json::Value;

use super::{draw_text_clipped, fill_rect, rgb565, scale};

#[rustfmt::skip]
pub(super) fn draw_status_indicators(frame: &mut [u8], snapshot: &Value, brightness: f32) {
    let cpu = snapshot.get("cpuLoadRatio").and_then(Value::as_f64).unwrap_or(0.0);
    let cpu_color = if cpu >= 0.85 {
        palette::PULSES
    } else if cpu >= 0.6 {
        palette::SPARKS
    } else {
        palette::SYSTEM
    };
    draw_cpu_icon(frame, 117, 5, rgb565(scale(cpu_color, brightness)));
}

#[rustfmt::skip]
pub(super) fn draw_footer(frame: &mut [u8], snapshot: &Value, brightness: f32) {
    let display = snapshot.get("display").unwrap_or(&Value::Null);
    let toast = display.get("toast").and_then(Value::as_str).unwrap_or_default();
    let text = rgb565(scale(palette::WHITE, brightness));
    if !toast.is_empty() {
        let background = rgb565(scale(palette::BLACK, brightness));
        fill_rect(frame, 0, 114, 128, 14, background);
        draw_text_clipped(frame, toast, 5, 118, 17, text);
        return;
    }
    draw_transport_icon(frame, snapshot, brightness);
    if snapshot.get("eventDotOn").and_then(Value::as_bool).unwrap_or(false) {
        let voice_steal = snapshot.get("voiceSteal").and_then(Value::as_bool).unwrap_or(false);
        let color = if voice_steal { palette::PULSES } else { palette::WHITE };
        let dot = rgb565(scale(color, brightness));
        fill_rect(frame, 119, 119, 5, 5, dot);
    }
}

#[rustfmt::skip]
fn draw_transport_icon(frame: &mut [u8], snapshot: &Value, brightness: f32) {
    let icon_name = snapshot.get("transportIcon").and_then(Value::as_str).unwrap_or("stop");
    let flash = snapshot.get("transportFlash").and_then(Value::as_str).unwrap_or("none");
    let rgb = match (icon_name, flash) {
        ("play", "measure") => palette::WORLDS,
        ("play", "beat") => palette::SPARKS,
        ("stop", _) => palette::PULSES,
        ("pause", _) => palette::TONES,
        _ => palette::WHITE,
    };
    draw_transport_shape(frame, icon_name, 101, 118, rgb565(scale(rgb, brightness)));
}

fn draw_cpu_icon(frame: &mut [u8], x: usize, y: usize, color: u16) {
    fill_rect(frame, x + 1, y + 1, 6, 6, color);
    fill_rect(frame, x + 3, y + 3, 2, 2, 0);
    fill_rect(frame, x, y + 2, 1, 1, color);
    fill_rect(frame, x, y + 5, 1, 1, color);
}

fn draw_transport_shape(frame: &mut [u8], icon: &str, x: usize, y: usize, color: u16) {
    match icon {
        "play" => {
            fill_rect(frame, x, y, 2, 9, color);
            fill_rect(frame, x + 2, y + 1, 2, 7, color);
            fill_rect(frame, x + 4, y + 2, 2, 5, color);
            fill_rect(frame, x + 6, y + 3, 2, 3, color);
            fill_rect(frame, x + 8, y + 4, 1, 1, color);
        }
        "pause" => {
            fill_rect(frame, x, y, 3, 8, color);
            fill_rect(frame, x + 6, y, 3, 8, color);
        }
        _ => fill_rect(frame, x, y, 8, 8, color),
    }
}
