use super::HardwareRenderCache;
use super::OledFramePublication;
use super::{OledOutputKey, OledOutputState};
use octessera_hal::OledSsd1351;
use std::ops::Range;
use std::time::{Duration, Instant};

pub(super) const OLED_RETRY_INTERVAL: Duration = Duration::from_millis(100);
const OLED_ERROR_LOG_INTERVAL: Duration = Duration::from_secs(1);
const OLED_ROW_BYTES: usize = 128 * 2;

pub(super) trait OledRenderDevice {
    fn display_on(&mut self) -> Result<(), String>;
    fn write_frame(&mut self, frame: &[u8]) -> Result<(), String>;
    /// Writes only `rows` of `frame`; the panel already shows the others.
    fn write_rows(&mut self, frame: &[u8], _rows: Range<usize>) -> Result<(), String> {
        self.write_frame(frame)
    }
    fn display_off(&mut self) -> Result<(), String>;
    fn set_master_contrast(&mut self, step: u8) -> Result<(), String>;
}

impl OledRenderDevice for OledSsd1351 {
    fn display_on(&mut self) -> Result<(), String> {
        OledSsd1351::display_on(self)
    }

    fn write_frame(&mut self, frame: &[u8]) -> Result<(), String> {
        OledSsd1351::write_frame(self, frame)
    }

    fn write_rows(&mut self, frame: &[u8], rows: Range<usize>) -> Result<(), String> {
        OledSsd1351::write_rows(self, frame, rows)
    }

    fn display_off(&mut self) -> Result<(), String> {
        OledSsd1351::display_off(self)
    }

    fn set_master_contrast(&mut self, step: u8) -> Result<(), String> {
        OledSsd1351::set_master_contrast(self, step)
    }
}

fn render_native_oled<O: OledRenderDevice>(
    oled: &mut O,
    off: bool,
    publication: &OledFramePublication,
    force_frame: bool,
    state: &mut OledOutputState,
) -> Result<bool, String> {
    let frame = publication.pixels();
    let frame_key = publication.revision();
    let master_contrast = publication.master_contrast();
    if state.master_contrast != Some(master_contrast) {
        oled.set_master_contrast(master_contrast)?;
        state.master_contrast = Some(master_contrast);
    }
    let display_changed = state.display_off != Some(off);
    if !off && display_changed {
        oled.display_on()?;
        state.display_off = Some(false);
    }
    let changed = match state.pixels.as_deref() {
        Some(shown) if !force_frame => changed_rows(shown, frame),
        _ => Some(0..frame.len() / OLED_ROW_BYTES),
    };
    let frame_written = changed.is_some();
    if let Some(rows) = changed {
        let written = if rows.len() * OLED_ROW_BYTES == frame.len() {
            oled.write_frame(frame)
        } else {
            oled.write_rows(frame, rows)
        };
        if let Err(error) = written {
            state.pixels = None;
            return Err(error);
        }
        state.pixels = Some(frame.to_vec());
    }
    state.frame = Some(frame_key);
    state.physical_revision = state.physical_revision.max(frame_key);
    if off && display_changed {
        oled.display_off()?;
        state.display_off = Some(true);
    }
    Ok(frame_written)
}

/// The first to the last row that differs between the frame on the panel and the new one.
fn changed_rows(shown: &[u8], frame: &[u8]) -> Option<Range<usize>> {
    if shown.len() != frame.len() {
        return Some(0..frame.len() / OLED_ROW_BYTES);
    }
    let differs = |row: &usize| {
        let bytes = row * OLED_ROW_BYTES..(row + 1) * OLED_ROW_BYTES;
        shown[bytes.clone()] != frame[bytes]
    };
    let rows = frame.len() / OLED_ROW_BYTES;
    let first = (0..rows).find(differs)?;
    let last = (first..rows).rev().find(differs)?;
    Some(first..last + 1)
}

pub(super) fn render_oled_if_changed_off<O: OledRenderDevice>(
    oled: &mut O,
    off: bool,
    publication: &OledFramePublication,
    cache: &mut HardwareRenderCache,
    now: Instant,
) -> Option<Instant> {
    let key = OledOutputKey::new(publication, off);
    if cache.oled_rendered_key == Some(key) {
        cache.clear_oled_retry();
        return None;
    }
    if let Some(retry_at) = cache.oled_retry_at {
        let retry_output_changed = cache.oled_retry_publication.as_ref() != Some(publication)
            || cache.oled_retry_display_off != key.display_off;
        if retry_output_changed {
            cache.oled_retry_publication = Some(publication.clone());
        }
        cache.oled_retry_display_off = key.display_off;
        if now < retry_at && !retry_output_changed {
            return Some(retry_at);
        }
    }
    attempt_oled_write(oled, off, publication, key, cache, now)
}

#[cfg(test)]
pub(crate) fn physical_oled_publication(
    publication: &OledFramePublication,
    cache: &HardwareRenderCache,
) -> Result<OledFramePublication, String> {
    let pixels = publication.pixels();
    let revision = physical_oled_revision(pixels, cache)?;
    OledFramePublication::native_pixels(
        revision,
        publication.shared_pixels(),
        publication.master_contrast(),
    )
}

pub(super) fn physical_oled_revision(
    pixels: &[u8],
    cache: &HardwareRenderCache,
) -> Result<u64, String> {
    let state = &cache.oled_output_state;
    if let (Some(accepted), Some(revision)) = (state.pixels.as_deref(), state.frame) {
        if accepted == pixels {
            return Ok(revision);
        }
    }
    state
        .physical_revision
        .checked_add(1)
        .ok_or_else(|| "physical OLED revision exhausted".into())
}

pub(crate) fn oled_publication_is_accepted(
    publication: &OledFramePublication,
    off: bool,
    cache: &HardwareRenderCache,
) -> bool {
    let key = OledOutputKey::new(publication, off);
    let frame = publication.pixels();
    cache.oled_rendered_key == Some(key)
        && cache.oled_output_state.frame == Some(publication.revision())
        && cache.oled_output_state.display_off == Some(off)
        && cache.oled_output_state.master_contrast == Some(publication.master_contrast())
        && cache.oled_output_state.pixels.as_deref() == Some(frame)
}

pub(crate) fn render_oled_typed_if_changed(
    oled: &mut super::OledRenderOutput,
    off: bool,
    publication: &OledFramePublication,
    cache: &mut HardwareRenderCache,
    now: Instant,
) -> Option<Instant> {
    render_oled_if_changed_off(oled, off, publication, cache, now)
}

pub(crate) fn retry_oled_if_due(
    oled: &mut super::OledRenderOutput,
    cache: &mut HardwareRenderCache,
    now: Instant,
) -> (Option<Instant>, bool) {
    let attempted = cache.oled_retry_at.is_some_and(|retry_at| now >= retry_at);
    (retry_oled_if_due_with_device(oled, cache, now), attempted)
}

fn retry_oled_if_due_with_device<O: OledRenderDevice>(
    oled: &mut O,
    cache: &mut HardwareRenderCache,
    now: Instant,
) -> Option<Instant> {
    let retry_at = cache.oled_retry_at?;
    if now < retry_at {
        return Some(retry_at);
    }
    let Some(publication) = cache.oled_retry_publication.clone() else {
        cache.clear_oled_retry();
        return None;
    };
    render_oled_if_changed_off(oled, cache.oled_retry_display_off, &publication, cache, now)
}

pub(super) fn force_oled_render_off<O: OledRenderDevice>(
    oled: &mut O,
    off: bool,
    publication: &OledFramePublication,
    cache: &mut HardwareRenderCache,
) -> Result<(), String> {
    cache.oled_output_state.display_off = None;
    cache.oled_output_state.master_contrast = None;
    let result = render_native_oled(oled, off, publication, true, &mut cache.oled_output_state);
    cache.clear_oled_retry();
    result.map(|frame_written| {
        cache.mark_oled_rendered(OledOutputKey::new(publication, off), frame_written);
    })
}

pub(crate) fn force_oled_typed(
    oled: &mut super::OledRenderOutput,
    off: bool,
    publication: &OledFramePublication,
    cache: &mut HardwareRenderCache,
) -> Result<(), String> {
    force_oled_render_off(oled, off, publication, cache)
}

fn attempt_oled_write<O: OledRenderDevice>(
    oled: &mut O,
    off: bool,
    publication: &OledFramePublication,
    key: OledOutputKey,
    cache: &mut HardwareRenderCache,
    now: Instant,
) -> Option<Instant> {
    match render_native_oled(oled, off, publication, false, &mut cache.oled_output_state) {
        Ok(frame_written) => {
            cache.mark_oled_rendered(key, frame_written);
            cache.clear_oled_retry();
            None
        }
        Err(error) => {
            if cache
                .oled_error_log_at
                .is_none_or(|next_log| now >= next_log)
            {
                eprintln!("pi OLED native frame write failed: {error}");
                cache.oled_error_log_at = Some(now + OLED_ERROR_LOG_INTERVAL);
            }
            cache.oled_retry_at = Some(now + OLED_RETRY_INTERVAL);
            cache.oled_retry_publication = Some(publication.clone());
            cache.oled_retry_display_off = off;
            cache.oled_retry_at
        }
    }
}

impl HardwareRenderCache {
    pub(crate) fn clear_oled_retry(&mut self) {
        self.oled_retry_at = None;
        self.oled_retry_publication = None;
        self.oled_retry_display_off = false;
        self.oled_error_log_at = None;
    }
}

#[cfg(test)]
mod row_window_tests;
#[cfg(test)]
mod tests;
