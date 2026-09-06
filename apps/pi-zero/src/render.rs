#![cfg_attr(feature = "hardware-orange-pi-zero-2w", allow(dead_code))]

use crate::oled_frame_cache::{OledFrameKey, OledFramePublication};
use crate::seesaw_io::SeesawCommand;
use octessera_hal::OledSsd1351;
use serde_json::Value;
use std::sync::mpsc::Sender;
use std::time::Instant;

pub(crate) mod hdmi;
mod oled;
mod oled_output;
mod oled_ownership;
mod ownership_control;
mod ownership_decision;
mod sleep_leds;

pub(crate) use oled::OLED_FRAME_BYTES;
#[cfg(test)]
use oled::{glyph_rows, oled_frame, oled_frame_into};
pub(crate) use oled_output::retry_oled_if_due;
use oled_output::{force_oled_render, render_oled_if_changed};
pub(crate) use oled_ownership::{
    handle_stage, restore, restore_after_dropped_ack, OledOwnershipStage, OledOwnershipState,
    OledRenderControl,
};
pub(crate) use ownership_control::{
    ownership_stage_for_render, restore_after_dropped_ack_for_render, restore_for_render,
};
pub(crate) use ownership_decision::{
    initial_snapshot_render_result, mark_handoff_failed_decision, retry_oled_decision,
    select_snapshot_render, snapshot_requires_oled_ack, SnapshotRenderDecision,
};
use sleep_leds::{SleepLedAnimation, SleepLedFrames};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct OledOutputKey {
    pub(super) frame: OledFrameKey,
    pub(super) display_off: bool,
}

impl OledOutputKey {
    pub(super) fn new(frame: OledFrameKey, display_off: bool) -> Self {
        Self { frame, display_off }
    }
}

#[derive(Default)]
pub(super) struct OledOutputState {
    pub(super) frame: Option<OledFrameKey>,
    pub(super) display_off: Option<bool>,
}

const SPLASH_BOOT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/splash_boot.rgb565"));
const SPLASH_SLEEP_SHUTDOWN: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/splash_sleep_shutdown.rgb565"));
const SLEEP_DIM_SCALE: f32 = 0.08;
const MIN_SLEEP_DIM_SCALE: f32 = 0.04;

pub(crate) fn shutdown_splash_base_frame() -> Vec<u8> {
    SPLASH_SLEEP_SHUTDOWN.to_vec()
}

#[path = "render/boot_sweep.rs"]
mod boot_sweep;
#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) use boot_sweep::{
    boot_sweep_base_frame, boot_sweep_bottom_row_origin, boot_sweep_deadline_offset_ns,
    boot_sweep_frame, boot_sweep_frame_from, boot_sweep_frames, logical_to_physical_bottom,
    physical_to_logical_input, rgb565_at, BOOT_SWEEP_BAND_WIDTH, BOOT_SWEEP_COLORS,
    BOOT_SWEEP_CYCLE_NS, BOOT_SWEEP_FRAMES, BOOT_SWEEP_LEAN_DENOMINATOR, BOOT_SWEEP_LEAN_NUMERATOR,
    BOOT_SWEEP_REST_CHECK_NS, BOOT_SWEEP_REST_NS, BOOT_SWEEP_SEPARATOR_COLOR,
    BOOT_SWEEP_SEPARATOR_WIDTH, BOOT_SWEEP_TRAIN_WIDTH,
};
#[cfg(all(not(test), not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) use boot_sweep::{
    boot_sweep_base_frame, boot_sweep_frames, BOOT_SWEEP_CYCLE_NS, BOOT_SWEEP_FRAMES,
    BOOT_SWEEP_REST_CHECK_NS, BOOT_SWEEP_REST_NS,
};
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(crate) use boot_sweep::{boot_sweep_deadline, render_boot_splash};

pub struct HardwareRenderTargets {
    pub oled: OledSsd1351,
    pub seesaw_tx: Sender<SeesawCommand>,
    pub oled_handoff: Option<crate::boot_oled_handoff::NativeOledGuard>,
    pub hdmi: hdmi::HdmiFramebuffer,
}

pub struct HardwareRenderCache {
    led_frame: Option<[[u8; 3]; 64]>,
    neokey_colors: Option<[[u8; 3]; 4]>,
    sleep_leds: SleepLedAnimation,
    oled_rendered_key: Option<OledOutputKey>,
    oled_output_state: OledOutputState,
    oled_render_count: u64,
    oled_retry_at: Option<Instant>,
    oled_retry_publication: Option<OledFramePublication>,
    oled_retry_display_off: bool,
    oled_error_log_at: Option<Instant>,
    hdmi_signature: u64,
}

impl HardwareRenderCache {
    pub fn new() -> Self {
        Self {
            led_frame: None,
            neokey_colors: None,
            sleep_leds: SleepLedAnimation::new(),
            oled_rendered_key: None,
            oled_output_state: OledOutputState::default(),
            oled_render_count: 0,
            oled_retry_at: None,
            oled_retry_publication: None,
            oled_retry_display_off: false,
            oled_error_log_at: None,
            hdmi_signature: 0,
        }
    }
}

impl Default for HardwareRenderCache {
    fn default() -> Self {
        Self::new()
    }
}

pub fn render_snapshot_cached(
    targets: &mut HardwareRenderTargets,
    snapshot: &Value,
    oled: &OledFramePublication,
    cache: &mut HardwareRenderCache,
) -> Option<Instant> {
    let oled_retry_deadline = render_oled_and_leds_cached(targets, snapshot, oled, cache);
    let hdmi_retry_deadline =
        render_hdmi_if_changed(&mut targets.hdmi, snapshot, cache, Instant::now());
    next_deadline(oled_retry_deadline, hdmi_retry_deadline)
}

pub(crate) fn render_oled_and_leds_cached(
    targets: &mut HardwareRenderTargets,
    snapshot: &Value,
    oled: &OledFramePublication,
    cache: &mut HardwareRenderCache,
) -> Option<Instant> {
    let animation_deadline = render_leds_at(targets, snapshot, cache, Instant::now());
    let oled_retry_deadline =
        render_oled_if_changed(&mut targets.oled, snapshot, oled, cache, Instant::now());
    next_deadline(animation_deadline, oled_retry_deadline)
}

pub(crate) fn retry_hdmi_if_due(
    targets: &mut HardwareRenderTargets,
    snapshot: &Value,
    cache: &mut HardwareRenderCache,
    now: Instant,
) -> Option<Instant> {
    render_hdmi_if_changed(&mut targets.hdmi, snapshot, cache, now)
}

fn render_hdmi_if_changed(
    hdmi: &mut hdmi::HdmiFramebuffer,
    snapshot: &Value,
    cache: &mut HardwareRenderCache,
    now: Instant,
) -> Option<Instant> {
    let signature = hdmi::hdmi_signature(snapshot);
    if cache.hdmi_signature == signature && !hdmi.has_pending_retry() {
        return None;
    }
    let outcome = hdmi.render(snapshot, now);
    if outcome.applied {
        cache.hdmi_signature = signature;
    }
    outcome.retry_at
}

pub(crate) fn render_leds_only(
    targets: &mut HardwareRenderTargets,
    snapshot: &Value,
    cache: &mut HardwareRenderCache,
    now: Instant,
) -> Option<Instant> {
    render_leds_at(targets, snapshot, cache, now)
}

fn render_leds_at(
    targets: &mut HardwareRenderTargets,
    snapshot: &Value,
    cache: &mut HardwareRenderCache,
    now: Instant,
) -> Option<Instant> {
    if snapshot_display_off(snapshot) {
        let settings = snapshot.get("settings").unwrap_or(&Value::Null);
        let entered_sleep = cache.sleep_leds.enter(
            now,
            brightness_scale(settings.get("gridBrightness")),
            brightness_scale(settings.get("buttonBrightness")),
        );
        if entered_sleep {
            let frames = cache.sleep_leds.frames_at(now);
            send_sleep_led_frames(targets, cache, frames);
        } else if let Some(frames) = cache.sleep_leds.frames_if_due(now) {
            send_sleep_led_frames(targets, cache, frames);
        }
        cache.sleep_leds.next_deadline()
    } else {
        if cache.sleep_leds.active() {
            cache.clear_sleep_animation();
        }
        render_normal_leds(targets, snapshot, cache);
        None
    }
}

pub(crate) fn force_latest_oled(
    targets: &mut HardwareRenderTargets,
    snapshot: &Value,
    oled: &OledFramePublication,
    cache: &mut HardwareRenderCache,
) -> Result<(), String> {
    force_oled_render(&mut targets.oled, snapshot, oled, cache)
}

impl HardwareRenderCache {
    pub(crate) fn oled_render_count(&self) -> u64 {
        self.oled_render_count
    }

    pub(super) fn mark_oled_rendered(&mut self, key: OledOutputKey) {
        self.oled_rendered_key = Some(key);
        self.oled_render_count = self.oled_render_count.saturating_add(1);
    }

    fn clear_sleep_animation(&mut self) {
        self.sleep_leds.stop();
        self.led_frame = None;
        self.neokey_colors = None;
    }

    pub(crate) fn render_sleep_tick(
        &mut self,
        targets: &mut HardwareRenderTargets,
        now: Instant,
    ) -> Option<Instant> {
        if !self.sleep_leds.active() {
            return None;
        }
        if let Some(frames) = self.sleep_leds.frames_if_due(now) {
            send_sleep_led_frames(targets, self, frames);
        }
        self.sleep_leds.next_deadline()
    }
}

fn render_normal_leds(
    targets: &mut HardwareRenderTargets,
    snapshot: &Value,
    cache: &mut HardwareRenderCache,
) {
    if let Some(frame) = led_frame(snapshot) {
        send_grid_frame(targets, cache, frame);
    }
    send_neokey_colors(targets, cache, neokey_colors(snapshot));
}

fn send_sleep_led_frames(
    targets: &mut HardwareRenderTargets,
    cache: &mut HardwareRenderCache,
    frames: SleepLedFrames,
) {
    send_grid_frame(targets, cache, frames.grid);
    send_neokey_colors(targets, cache, frames.keys);
}

fn send_grid_frame(
    targets: &mut HardwareRenderTargets,
    cache: &mut HardwareRenderCache,
    frame: [[u8; 3]; 64],
) {
    if cache.led_frame.as_ref() != Some(&frame)
        && targets
            .seesaw_tx
            .send(SeesawCommand::GridFrame(frame))
            .is_ok()
    {
        cache.led_frame = Some(frame);
    }
}

fn send_neokey_colors(
    targets: &mut HardwareRenderTargets,
    cache: &mut HardwareRenderCache,
    colors: [[u8; 3]; 4],
) {
    let previous = cache.neokey_colors.unwrap_or([[u8::MAX; 3]; 4]);
    if previous != colors
        && targets
            .seesaw_tx
            .send(SeesawCommand::NeoKeyColors(colors))
            .is_ok()
    {
        cache.neokey_colors = Some(colors);
    }
}

pub fn led_frame(snapshot: &Value) -> Option<[[u8; 3]; 64]> {
    let settings = snapshot.get("settings").unwrap_or(&Value::Null);
    let mut brightness = brightness_scale(settings.get("gridBrightness"));
    if settings
        .get("ledsDimmed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        brightness = sleep_dim_brightness(brightness);
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
    let brightness = settings
        .get("buttonBrightness")
        .and_then(Value::as_u64)
        .unwrap_or(100)
        .min(100) as u32;
    let dimmed = settings
        .get("ledsDimmed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let basis_points = if dimmed {
        if brightness == 0 {
            0
        } else {
            (brightness * 8).max(400)
        }
    } else {
        brightness * 100
    };
    let leds = snapshot.get("neoKeyLeds").expect("native NeoKey LEDs");
    ["back", "space", "shift", "fn"].map(|key| {
        let rgb = leds
            .get(key)
            .and_then(Value::as_array)
            .expect("native NeoKey LED color");
        [0, 1, 2].map(|index| {
            let channel = rgb[index].as_u64().expect("native NeoKey channel") as u32;
            ((channel * basis_points + 5_000) / 10_000).min(255) as u8
        })
    })
}

pub(crate) fn next_deadline(first: Option<Instant>, second: Option<Instant>) -> Option<Instant> {
    match (first, second) {
        (Some(first), Some(second)) => Some(first.min(second)),
        (Some(deadline), None) | (None, Some(deadline)) => Some(deadline),
        (None, None) => None,
    }
}

pub fn render_shutdown_splash(oled: &mut OledSsd1351) {
    let snapshot = serde_json::json!({
        "display": {
            "off": false,
            "splash": "shutdown",
            "toast": ""
        },
        "settings": { "displayBrightness": 100 }
    });
    let mut frame = vec![0_u8; OLED_FRAME_BYTES];
    let result = oled.display_on().and_then(|()| {
        oled::oled_frame_into(&snapshot, &mut frame);
        oled.write_frame(&frame)
    });
    if let Err(error) = result {
        eprintln!("pi OLED shutdown splash render failed: {error}");
    }
}

fn snapshot_display_off(snapshot: &Value) -> bool {
    snapshot
        .get("display")
        .and_then(|display| display.get("off"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
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

fn sleep_dim_brightness(brightness: f32) -> f32 {
    if brightness <= 0.0 {
        0.0
    } else {
        (brightness * SLEEP_DIM_SCALE).max(MIN_SLEEP_DIM_SCALE)
    }
}

#[rustfmt::skip]
pub(super) fn scale(rgb: [u8; 3], factor: f32) -> [u8; 3] { [
    ((rgb[0] as f32) * factor).round().clamp(0.0, 255.0) as u8,
    ((rgb[1] as f32) * factor).round().clamp(0.0, 255.0) as u8,
    ((rgb[2] as f32) * factor).round().clamp(0.0, 255.0) as u8,
] }

pub(super) fn dim(rgb: [u8; 3], divisor: u8) -> [u8; 3] {
    let divisor = divisor.max(1);
    [rgb[0] / divisor, rgb[1] / divisor, rgb[2] / divisor]
}

pub(super) fn rgb565(rgb: [u8; 3]) -> u16 {
    ((u16::from(rgb[0]) & 0xF8) << 8) | ((u16::from(rgb[1]) & 0xFC) << 3) | (u16::from(rgb[2]) >> 3)
}

#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
#[path = "render/boot_sweep_tests.rs"]
mod boot_sweep_tests;
#[cfg(test)]
#[path = "render/footer_tests.rs"]
mod footer_tests;
#[cfg(test)]
#[path = "render/hdmi_cache_tests.rs"]
mod hdmi_cache_tests;
#[cfg(test)]
#[path = "render/native_frame_sleep_wake_tests.rs"]
mod native_frame_sleep_wake_tests;
#[cfg(test)]
#[path = "render/oled_error_tests.rs"]
mod oled_error_tests;
#[cfg(test)]
#[path = "render/oled_glyph_tests.rs"]
mod oled_glyph_tests;
#[cfg(test)]
#[path = "render/oled_parity_tests.rs"]
mod oled_parity_tests;
#[cfg(test)]
#[path = "render/oled_test_adapter.rs"]
mod oled_test_adapter;
#[cfg(test)]
mod tests;
