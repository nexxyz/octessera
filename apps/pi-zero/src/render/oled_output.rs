use super::oled::{oled_frame_into, oled_signature};
use super::HardwareRenderCache;
use octessera_hal::OledSsd1351;
use serde_json::Value;
use std::time::{Duration, Instant};

pub(super) const OLED_RETRY_INTERVAL: Duration = Duration::from_millis(100);
const OLED_ERROR_LOG_INTERVAL: Duration = Duration::from_secs(1);

pub(super) trait OledRenderDevice {
    fn display_on(&mut self) -> Result<(), String>;
    fn write_frame(&mut self, frame: &[u8]) -> Result<(), String>;
    fn display_off(&mut self) -> Result<(), String>;
}

impl OledRenderDevice for OledSsd1351 {
    fn display_on(&mut self) -> Result<(), String> {
        OledSsd1351::display_on(self)
    }

    fn write_frame(&mut self, frame: &[u8]) -> Result<(), String> {
        OledSsd1351::write_frame(self, frame)
    }

    fn display_off(&mut self) -> Result<(), String> {
        OledSsd1351::display_off(self)
    }
}

pub(super) fn render_oled<O: OledRenderDevice>(
    oled: &mut O,
    snapshot: &Value,
    frame: &mut [u8],
) -> Result<(), String> {
    let off = super::snapshot_display_off(snapshot);
    if !off {
        oled.display_on()?;
    }
    oled_frame_into(snapshot, frame);
    let frame_result = oled.write_frame(frame);
    let display_result = if off { oled.display_off() } else { Ok(()) };
    frame_result.and(display_result)
}

pub(super) fn render_oled_if_changed<O: OledRenderDevice>(
    oled: &mut O,
    snapshot: &Value,
    cache: &mut HardwareRenderCache,
    now: Instant,
) -> Option<Instant> {
    let signature = oled_signature(snapshot);
    if cache.has_rendered_oled() && cache.oled_signature == signature {
        cache.clear_oled_retry();
        return None;
    }
    if let Some(retry_at) = cache.oled_retry_at {
        if cache.oled_retry_signature != Some(signature) {
            cache.oled_retry_signature = Some(signature);
            cache.oled_retry_snapshot = Some(snapshot.clone());
        }
        if now < retry_at {
            return Some(retry_at);
        }
    }
    attempt_oled_render(oled, snapshot, signature, cache, now)
}

pub(crate) fn retry_oled_if_due(
    oled: &mut OledSsd1351,
    cache: &mut HardwareRenderCache,
    now: Instant,
) -> Option<Instant> {
    let retry_at = cache.oled_retry_at?;
    if now < retry_at {
        return Some(retry_at);
    }
    let Some(snapshot) = cache.oled_retry_snapshot.clone() else {
        cache.clear_oled_retry();
        return None;
    };
    render_oled_if_changed(oled, &snapshot, cache, now)
}

fn attempt_oled_render<O: OledRenderDevice>(
    oled: &mut O,
    snapshot: &Value,
    signature: u64,
    cache: &mut HardwareRenderCache,
    now: Instant,
) -> Option<Instant> {
    match render_oled(oled, snapshot, &mut cache.oled_frame) {
        Ok(()) => {
            cache.oled_signature = signature;
            cache.mark_oled_rendered();
            cache.clear_oled_retry();
            None
        }
        Err(error) => {
            if cache
                .oled_error_log_at
                .is_none_or(|next_log| now >= next_log)
            {
                eprintln!("pi OLED render failed: {error}");
                cache.oled_error_log_at = Some(now + OLED_ERROR_LOG_INTERVAL);
            }
            cache.oled_retry_at = Some(now + OLED_RETRY_INTERVAL);
            cache.oled_retry_signature = Some(signature);
            cache.oled_retry_snapshot = Some(snapshot.clone());
            cache.oled_retry_at
        }
    }
}

impl HardwareRenderCache {
    fn clear_oled_retry(&mut self) {
        self.oled_retry_at = None;
        self.oled_retry_signature = None;
        self.oled_retry_snapshot = None;
        self.oled_error_log_at = None;
    }
}
