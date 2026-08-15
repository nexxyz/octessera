use super::HardwareRenderCache;
use super::{OledOutputKey, OledOutputState};
use crate::oled_frame_cache::OledFrameKey;
use crate::oled_frame_cache::OledFramePublication;
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

fn render_native_oled<O: OledRenderDevice>(
    oled: &mut O,
    snapshot: &Value,
    frame: &[u8],
    frame_key: OledFrameKey,
    force_frame: bool,
    state: &mut OledOutputState,
) -> Result<(), String> {
    let off = super::snapshot_display_off(snapshot);
    let display_changed = state.display_off != Some(off);
    if !off && display_changed {
        oled.display_on()?;
        state.display_off = Some(false);
    }
    if force_frame || state.frame != Some(frame_key) {
        oled.write_frame(frame)?;
        state.frame = Some(frame_key);
    }
    if off && display_changed {
        oled.display_off()?;
        state.display_off = Some(true);
    }
    Ok(())
}

pub(super) fn render_oled_if_changed<O: OledRenderDevice>(
    oled: &mut O,
    snapshot: &Value,
    publication: &OledFramePublication,
    cache: &mut HardwareRenderCache,
    now: Instant,
) -> Option<Instant> {
    let key = OledOutputKey::new(publication.key(), super::snapshot_display_off(snapshot));
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
    attempt_oled_write(oled, snapshot, publication, key, cache, now)
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
    let Some(publication) = cache.oled_retry_publication.clone() else {
        cache.clear_oled_retry();
        return None;
    };
    let snapshot = if cache.oled_retry_display_off {
        serde_json::json!({"display": {"off": true}})
    } else {
        Value::Null
    };
    render_oled_if_changed(oled, &snapshot, &publication, cache, now)
}

pub(crate) fn force_oled_render(
    oled: &mut OledSsd1351,
    snapshot: &Value,
    publication: &OledFramePublication,
    cache: &mut HardwareRenderCache,
) -> Result<(), String> {
    force_oled_render_with_device(oled, snapshot, publication, cache)
}

fn force_oled_render_with_device<O: OledRenderDevice>(
    oled: &mut O,
    snapshot: &Value,
    publication: &OledFramePublication,
    cache: &mut HardwareRenderCache,
) -> Result<(), String> {
    cache.oled_output_state.display_off = None;
    let result = write_publication(
        oled,
        snapshot,
        publication,
        true,
        &mut cache.oled_output_state,
    );
    cache.clear_oled_retry();
    result.map(|()| {
        cache.mark_oled_rendered(OledOutputKey::new(
            publication.key(),
            super::snapshot_display_off(snapshot),
        ));
    })
}

fn attempt_oled_write<O: OledRenderDevice>(
    oled: &mut O,
    snapshot: &Value,
    publication: &OledFramePublication,
    key: OledOutputKey,
    cache: &mut HardwareRenderCache,
    now: Instant,
) -> Option<Instant> {
    match write_publication(
        oled,
        snapshot,
        publication,
        false,
        &mut cache.oled_output_state,
    ) {
        Ok(()) => {
            cache.mark_oled_rendered(key);
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
            cache.oled_retry_display_off = super::snapshot_display_off(snapshot);
            cache.oled_retry_at
        }
    }
}

fn write_publication<O: OledRenderDevice>(
    oled: &mut O,
    snapshot: &Value,
    publication: &OledFramePublication,
    force_frame: bool,
    state: &mut OledOutputState,
) -> Result<(), String> {
    let black = [0_u8; super::OLED_FRAME_BYTES];
    let frame = publication.pixels().unwrap_or(&black);
    render_native_oled(oled, snapshot, frame, publication.key(), force_frame, state)
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
mod tests {
    use super::*;
    use crate::oled_frame_cache::OledFramePublication;
    use serde_json::json;

    struct FakeOled {
        writes: Vec<Vec<u8>>,
    }

    impl FakeOled {
        fn new() -> Self {
            Self { writes: Vec::new() }
        }
    }

    impl OledRenderDevice for FakeOled {
        fn display_on(&mut self) -> Result<(), String> {
            Ok(())
        }

        fn write_frame(&mut self, frame: &[u8]) -> Result<(), String> {
            self.writes.push(frame.to_vec());
            Ok(())
        }

        fn display_off(&mut self) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn force_render_writes_cached_revision_again_with_supplied_pixels() {
        let snapshot = json!({
            "display": { "off": false },
            "oledFrameRevision": 7
        });
        let pixels = vec![0x2a; super::super::OLED_FRAME_BYTES];
        let publication = OledFramePublication::test_native(7, pixels.clone());
        let mut cache = HardwareRenderCache::default();
        let mut oled = FakeOled::new();

        assert_eq!(
            render_oled_if_changed(
                &mut oled,
                &snapshot,
                &publication,
                &mut cache,
                Instant::now(),
            ),
            None
        );
        assert_eq!(oled.writes.len(), 1);

        force_oled_render_with_device(&mut oled, &snapshot, &publication, &mut cache).unwrap();

        assert_eq!(oled.writes, vec![pixels.clone(), pixels]);
    }
}
