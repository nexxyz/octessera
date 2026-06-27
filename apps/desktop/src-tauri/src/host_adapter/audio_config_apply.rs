use super::DesktopPlaybackHostAdapter;

impl DesktopPlaybackHostAdapter {
    pub(super) fn handle_full_audio_config(
        &mut self,
        revision: u64,
        config: serde_json::Value,
    ) -> Result<(), String> {
        self.audio
            .audio_control
            .enqueue_full_config(revision, config)
    }
}
