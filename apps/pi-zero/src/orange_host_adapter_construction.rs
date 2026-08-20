use super::OrangeHostAdapter;
use crate::audio::AudioService;
use crate::main_paths::{default_samples_dir, default_store_dir};
use crate::midi_host::MidiHost;
use crate::oled_frame_cache::OledFrameCache;
use crate::orange_audio::OrangeAudioHost;
use crate::platform_service::PiPlatformService;
use playback_runtime::DeferredDefaultSave;
use std::path::{Path, PathBuf};
use std::sync::Arc;

impl OrangeHostAdapter {
    pub(crate) fn new(
        audio: AudioService,
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
        usb_midi_out_enabled: bool,
    ) -> Result<Self, String> {
        Self::with_directories(
            audio,
            default_store_dir(),
            default_samples_dir(),
            midi_in_handler,
            usb_midi_out_enabled,
        )
    }

    pub(crate) fn with_directories(
        audio: AudioService,
        store_dir: PathBuf,
        samples_dir: PathBuf,
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
        usb_midi_out_enabled: bool,
    ) -> Result<Self, String> {
        let store_dir = prepare_directory(&store_dir, "Orange store")?;
        let samples_dir = prepare_directory(&samples_dir, "Orange samples")?;
        let platform_service = PiPlatformService::new(store_dir.clone(), samples_dir.clone());
        platform_service.set_restore_preflight(Arc::new(orange_restore_recording_preflight));
        Ok(Self {
            audio: audio.clone(),
            audio_host: OrangeAudioHost::new(audio, samples_dir.clone()),
            platform_service,
            pending_default_save: DeferredDefaultSave::default(),
            pending_default_save_generation: None,
            midi: MidiHost::new(midi_in_handler, usb_midi_out_enabled),
            oled_frame_cache: OledFrameCache::default(),
            shutdown_request: None,
        })
    }

    #[cfg(all(test, any(unix, windows)))]
    pub(crate) fn with_setup_environment(
        audio: AudioService,
        store_dir: PathBuf,
        samples_dir: PathBuf,
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
        usb_midi_out_enabled: bool,
        environment: crate::setup_portal::SetupPortalEnvironment,
    ) -> Result<Self, String> {
        let store_dir = prepare_directory(&store_dir, "Orange store")?;
        let samples_dir = prepare_directory(&samples_dir, "Orange samples")?;
        let platform_service = PiPlatformService::new_with_setup_environment(
            store_dir.clone(),
            samples_dir.clone(),
            environment,
        );
        platform_service.set_restore_preflight(Arc::new(orange_restore_recording_preflight));
        Ok(Self {
            audio: audio.clone(),
            audio_host: OrangeAudioHost::new(audio, samples_dir.clone()),
            platform_service,
            pending_default_save: DeferredDefaultSave::default(),
            pending_default_save_generation: None,
            midi: MidiHost::new(midi_in_handler, usb_midi_out_enabled),
            oled_frame_cache: OledFrameCache::default(),
            shutdown_request: None,
        })
    }
}

fn prepare_directory(path: &Path, label: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(path)
        .map_err(|error| format!("{label} directory is not usable: {error}"))?;
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("{label} directory cannot be inspected: {error}"))?;
    if !metadata.is_dir() {
        return Err(format!(
            "{label} path is not a directory: {}",
            path.display()
        ));
    }
    path.canonicalize()
        .map_err(|error| format!("{label} directory cannot be resolved: {error}"))
}

fn orange_restore_recording_preflight() -> Result<(), String> {
    Ok(())
}
