use super::PiPlaybackHostAdapter;
use crate::oled_frame_cache::OledFramePublication;
use serde_json::Value;

impl PiPlaybackHostAdapter {
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

    pub(crate) fn oled_frame_fault(&self) -> Option<crate::oled_frame_cache::OledFrameCacheFault> {
        self.oled_frame_cache.fault()
    }

    pub(crate) fn submit_accepted_oled_frame(&self) -> Result<(), String> {
        let Some(frame) = self.oled_frame_cache.accepted_frame() else {
            return Ok(());
        };
        let Some(audio) = &self.audio else {
            return Ok(());
        };
        audio.submit_accepted_oled_frame(frame.revision(), frame.pixels())
    }

    pub(crate) fn poll_recording_status(&self) -> Option<playback_runtime::RuntimeStoreResult> {
        self.audio
            .as_ref()
            .and_then(crate::audio::AudioService::poll_recording_status)
    }
}
