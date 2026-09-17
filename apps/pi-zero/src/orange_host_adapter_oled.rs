use super::OrangeHostAdapter;
use crate::oled_frame_cache::{OledFrameCacheFault, OledFramePublication};
use serde_json::Value;

impl OrangeHostAdapter {
    pub(crate) fn ingest_oled_frame(&mut self, message: &playback_runtime::RunnerMessage) {
        self.oled_frame_cache.ingest(message);
    }

    pub(crate) fn accept_oled_frame_reference(&mut self, snapshot: &Value) {
        let _ = self.oled_frame_cache.accept_reference_value(snapshot);
    }

    pub(crate) fn oled_publication_for_snapshot(
        &mut self,
        snapshot: &Value,
        initial: bool,
    ) -> Result<OledFramePublication, String> {
        self.oled_frame_cache
            .publication_for_snapshot(snapshot, initial)
    }

    pub(crate) fn oled_frame_fault(&self) -> Option<OledFrameCacheFault> {
        self.oled_frame_cache.fault()
    }

    pub(crate) fn submit_accepted_oled_frame(&self) -> Result<(), String> {
        let Some(frame) = self.oled_frame_cache.accepted_frame() else {
            return Ok(());
        };
        self.audio
            .submit_accepted_oled_frame(frame.revision(), frame.pixels())
    }

    pub(crate) fn poll_recording_status(&self) -> Option<playback_runtime::RuntimeStoreResult> {
        self.audio.poll_recording_status()
    }
}
