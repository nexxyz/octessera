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
            pending_default_save: DeferredDefaultSave::default(),
            midi: MidiHost::new(midi_in_handler, usb_midi_out_enabled),
            usb_midi_out_enabled,
            usb_audio_out,
            power_request: None,
            latest_recovery_payload: None,
            oled_frame_cache: OledFrameCache::default(),
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

    #[cfg(all(test, any(unix, windows)))]
    pub(crate) fn new_with_setup_environment(
        audio: Option<AudioService>,
        store_dir: PathBuf,
        samples_dir: PathBuf,
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
        usb_midi_out_enabled: bool,
        usb_audio_out: UsbAudioOut,
        environment: crate::setup_portal::SetupPortalEnvironment,
    ) -> Self {
        let platform_service = PiPlatformService::new_with_setup_environment(
            store_dir.clone(),
            samples_dir.clone(),
            environment,
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
