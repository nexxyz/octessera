use super::*;
#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
use crate::usb_config::UsbAudioOut;

impl PiPlaybackHostAdapter {
    pub(super) fn with_platform_service(
        audio: Option<AudioService>,
        samples_dir: PathBuf,
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
        usb_midi_out_enabled: bool,
        audio_outputs: AudioOutputSet,
        platform_service: PiPlatformService,
    ) -> Self {
        let restore_audio = audio.clone();
        platform_service.set_restore_preflight(Arc::new(move || {
            if let Some(audio) = &restore_audio {
                audio.prepare_restore()?;
            }
            Ok(())
        }));
        Self {
            audio,
            samples_dir,
            platform_service,
            pending_default_save: DeferredDefaultSave::default(),
            pending_default_save_generation: None,
            midi: MidiHost::new(midi_in_handler, usb_midi_out_enabled),
            usb_midi_out_enabled,
            audio_outputs,
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
            samples_dir,
            midi_in_handler,
            usb_midi_out_enabled,
            usb_audio_out.outputs(),
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
            samples_dir,
            midi_in_handler,
            usb_midi_out_enabled,
            usb_audio_out.outputs(),
            platform_service,
        )
    }
}
