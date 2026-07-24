use super::*;

impl PiPlaybackHostAdapter {
    pub(super) fn with_platform_service(
        audio: Option<AudioService>,
        store_dir: PathBuf,
        samples_dir: PathBuf,
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
        usb_midi_out_enabled: bool,
        usb_audio_out: UsbAudioOut,
        platform_service: PiPlatformService,
    ) -> Self {
        Self {
            audio,
            store_dir,
            samples_dir,
            platform_service,
            pending_default_save: None,
            midi_out: None,
            midi_in: None,
            midi_in_handler,
            selected_midi_output_id: None,
            selected_midi_input_id: None,
            usb_midi_out_enabled,
            usb_audio_out,
            power_request: None,
            latest_recovery_payload: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_with_update_executor(
        audio: Option<AudioService>,
        store_dir: PathBuf,
        samples_dir: PathBuf,
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
        usb_midi_out_enabled: bool,
        usb_audio_out: UsbAudioOut,
        update_executor: Arc<dyn crate::device_update::UpdateExecutor>,
    ) -> Self {
        let platform_service = PiPlatformService::new_with_update_executor(
            store_dir.clone(),
            samples_dir.clone(),
            update_executor,
        );
        Self::with_platform_service(
            audio,
            store_dir,
            samples_dir,
            midi_in_handler,
            usb_midi_out_enabled,
            usb_audio_out,
            platform_service,
        )
    }
}
