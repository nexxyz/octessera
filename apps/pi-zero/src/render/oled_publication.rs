use playback_runtime::oled_frame::OLED_FRAME_BYTES;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OledFramePublication {
    revision: u64,
    pixels: Arc<[u8]>,
}

impl OledFramePublication {
    pub(crate) fn native_pixels(revision: u64, pixels: Vec<u8>) -> Result<Self, String> {
        if revision == 0 || pixels.len() != OLED_FRAME_BYTES {
            return Err("native OLED frame has invalid revision or pixel length".into());
        }
        Ok(Self {
            revision,
            pixels: Arc::from(pixels),
        })
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub(crate) fn shared_pixels(&self) -> Arc<[u8]> {
        Arc::clone(&self.pixels)
    }

    #[cfg(test)]
    pub(crate) fn test_native(revision: u64, pixels: Vec<u8>) -> Self {
        Self {
            revision,
            pixels: Arc::from(pixels),
        }
    }
}
