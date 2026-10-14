use super::HardwareRenderCache;
use super::OledFramePublication;
use super::{OledOutputKey, OledOutputState};
use octessera_hal::OledSsd1351;
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

fn render_native_oled<O: OledRenderDevice>(
    oled: &mut O,
    off: bool,
    frame: &[u8],
    frame_key: u64,
    force_frame: bool,
    state: &mut OledOutputState,
) -> Result<bool, String> {
    let display_changed = state.display_off != Some(off);
    if !off && display_changed {
        oled.display_on()?;
        state.display_off = Some(false);
    }
    let frame_written = force_frame || state.pixels.as_deref() != Some(frame);
    if frame_written {
        oled.write_frame(frame)?;
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

pub(super) fn render_oled_if_changed_off<O: OledRenderDevice>(
    oled: &mut O,
    off: bool,
    publication: &OledFramePublication,
    cache: &mut HardwareRenderCache,
    now: Instant,
) -> Option<Instant> {
    let key = OledOutputKey::new(publication.revision(), off);
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
    OledFramePublication::native_pixels(revision, pixels.to_vec())
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
    let key = OledOutputKey::new(publication.revision(), off);
    let frame = publication.pixels();
    cache.oled_rendered_key == Some(key)
        && cache.oled_output_state.frame == Some(publication.revision())
        && cache.oled_output_state.display_off == Some(off)
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
    let result = write_publication(oled, off, publication, true, &mut cache.oled_output_state);
    cache.clear_oled_retry();
    result.map(|frame_written| {
        cache.mark_oled_rendered(
            OledOutputKey::new(publication.revision(), off),
            frame_written,
        );
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
    match write_publication(oled, off, publication, false, &mut cache.oled_output_state) {
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

fn write_publication<O: OledRenderDevice>(
    oled: &mut O,
    off: bool,
    publication: &OledFramePublication,
    force_frame: bool,
    state: &mut OledOutputState,
) -> Result<bool, String> {
    render_native_oled(
        oled,
        off,
        publication.pixels(),
        publication.revision(),
        force_frame,
        state,
    )
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
mod tests;
