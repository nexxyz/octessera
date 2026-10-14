use crate::seesaw_io::SeesawCommand;
use octessera_hal::OledSsd1351;
use serde_json::Value;
use std::sync::mpsc::Sender;
use std::time::Instant;

pub(crate) mod hdmi;
mod native_leds;
mod native_publication;
pub(crate) use native_publication::{prepare_native_scene, LatestPresentation};
mod oled;
mod oled_output;
mod oled_publication;
pub(crate) use oled_publication::OledFramePublication;
mod oled_ownership;
mod ownership_control;
mod ownership_decision;
mod sleep_leds;

#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) mod test_oled_output;

#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) type OledRenderOutput = test_oled_output::TestOledOutput;
#[cfg(not(all(test, not(feature = "hardware-orange-pi-zero-2w"))))]
pub(crate) type OledRenderOutput = OledSsd1351;

pub(crate) use oled::OLED_FRAME_BYTES;
#[cfg(test)]
use oled::{glyph_rows, oled_frame, oled_frame_into};
pub(crate) use oled_output::retry_oled_if_due;
#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
use oled_output::OledRenderDevice;
pub(crate) use oled_ownership::{
    handle_stage, restore, restore_after_dropped_ack, OledOwnershipStage, OledOwnershipState,
    OledRenderControl,
};
pub(crate) use ownership_control::{
    ownership_stage_for_render, restore_after_dropped_ack_for_render, restore_for_render,
};
pub(crate) use ownership_decision::{
    mark_handoff_failed_decision, retry_oled_decision, select_snapshot_render,
    SnapshotRenderDecision,
};
use sleep_leds::{SleepLedAnimation, SleepLedFrames};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct OledOutputKey {
    pub(super) frame: u64,
    pub(super) display_off: bool,
}

impl OledOutputKey {
    pub(super) fn new(frame: u64, display_off: bool) -> Self {
        Self { frame, display_off }
    }
}

#[derive(Default)]
pub(super) struct OledOutputState {
    pub(super) frame: Option<u64>,
    pub(super) display_off: Option<bool>,
    pub(super) pixels: Option<Vec<u8>>,
    pub(super) physical_revision: u64,
}

const SPLASH_BOOT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/splash_boot.rgb565"));
const SPLASH_SLEEP_SHUTDOWN: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/splash_sleep_shutdown.rgb565"));
const SLEEP_DIM_SCALE: f32 = 0.08;
const MIN_SLEEP_DIM_SCALE: f32 = 0.04;

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn shutdown_splash_base_frame() -> Vec<u8> {
    SPLASH_SLEEP_SHUTDOWN.to_vec()
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
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
    pub oled: OledRenderOutput,
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
    hdmi_initialized: bool,
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
            hdmi_initialized: false,
        }
    }
}

impl Default for HardwareRenderCache {
    fn default() -> Self {
        Self::new()
    }
}

impl HardwareRenderCache {
    pub(crate) fn oled_render_count(&self) -> u64 {
        self.oled_render_count
    }

    pub(super) fn mark_oled_rendered(&mut self, key: OledOutputKey, frame_written: bool) {
        self.oled_rendered_key = Some(key);
        if frame_written {
            self.oled_render_count = self.oled_render_count.saturating_add(1);
        }
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

fn render_hdmi_if_changed(
    hdmi: &mut hdmi::HdmiFramebuffer,
    presentation: &playback_runtime::NativeHdmiPresentation,
    cache: &mut HardwareRenderCache,
    now: Instant,
) -> Option<Instant> {
    let signature = hdmi::hdmi_signature(presentation);
    if cache.hdmi_initialized && signature == cache.hdmi_signature && !hdmi.has_pending_retry() {
        return None;
    }
    let result = hdmi.render(presentation, now);
    if result.applied {
        cache.hdmi_signature = signature;
        cache.hdmi_initialized = true;
    }
    result.retry_at
}

pub(crate) fn next_deadline(first: Option<Instant>, second: Option<Instant>) -> Option<Instant> {
    match (first, second) {
        (Some(first), Some(second)) => Some(first.min(second)),
        (Some(deadline), None) | (None, Some(deadline)) => Some(deadline),
        (None, None) => None,
    }
}

pub fn render_shutdown_splash(oled: &mut OledRenderOutput) {
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

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn fault_oled_frame_into(lines: &[String], frame: &mut [u8], lit: bool) {
    oled::fault_frame_into(lines, frame, lit);
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
mod boot_sweep_tests;
#[cfg(test)]
mod footer_tests;
#[cfg(test)]
mod hdmi_cache_tests;
#[cfg(test)]
mod native_frame_sleep_wake_tests;
#[cfg(test)]
mod oled_error_tests;
#[cfg(test)]
mod oled_glyph_tests;
#[cfg(test)]
mod oled_parity_tests;
#[cfg(test)]
mod oled_test_adapter;
#[cfg(test)]
mod tests;
