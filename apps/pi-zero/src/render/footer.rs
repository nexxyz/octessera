use serde_json::Value;

use super::{draw_text, draw_text_clipped, fill_rect, rgb565, scale};

#[rustfmt::skip]
pub(super) fn draw_status_indicators(frame: &mut [u8], snapshot: &Value, brightness: f32) {
    let settings = snapshot.get("settings").unwrap_or(&Value::Null);
    let save = settings.get("autoSaveFlash").and_then(Value::as_str).unwrap_or("none") == "flash";
    let cpu = snapshot.get("cpuLoadRatio").and_then(Value::as_f64).unwrap_or(0.0);
    let save_color = if save { [255, 243, 176] } else { [51, 68, 51] };
    let cpu_color = if cpu >= 0.85 {
        [255, 102, 102]
    } else if cpu >= 0.6 {
        [255, 209, 102]
    } else {
        [51, 85, 68]
    };
    draw_text(frame, "S", 108, 5, 1, rgb565(scale(save_color, brightness)));
    draw_text(frame, "C", 117, 5, 1, rgb565(scale(cpu_color, brightness)));
}

#[rustfmt::skip]
pub(super) fn draw_footer(frame: &mut [u8], snapshot: &Value, brightness: f32) {
    let display = snapshot.get("display").unwrap_or(&Value::Null);
    let toast = display.get("toast").and_then(Value::as_str).unwrap_or_default();
    let text = rgb565(scale([215, 255, 232], brightness));
    if !toast.is_empty() {
        let background = rgb565(scale([6, 18, 13], brightness));
        fill_rect(frame, 0, 114, 128, 14, background);
        draw_text_clipped(frame, toast, 5, 118, 17, text);
        return;
    }
    draw_transport_icon(frame, snapshot, brightness);
    if snapshot.get("eventDotOn").and_then(Value::as_bool).unwrap_or(false) {
        let voice_steal = snapshot.get("voiceSteal").and_then(Value::as_bool).unwrap_or(false);
        let color = if voice_steal { [255, 51, 51] } else { [255, 220, 70] };
        let dot = rgb565(scale(color, brightness));
        fill_rect(frame, 119, 119, 5, 5, dot);
    }
}

#[rustfmt::skip]
fn draw_transport_icon(frame: &mut [u8], snapshot: &Value, brightness: f32) {
    let icon = match snapshot.get("transportIcon").and_then(Value::as_str).unwrap_or("stop") {
        "play" => ">",
        "pause" => "||",
        _ => "[]",
    };
    let flash = snapshot.get("transportFlash").and_then(Value::as_str).unwrap_or("none");
    let rgb = match flash {
        "measure" => [255, 51, 51],
        "beat" => [51, 255, 102],
        _ => [215, 255, 232],
    };
    draw_text(frame, icon, 101, 118, 1, rgb565(scale(rgb, brightness)));
}
