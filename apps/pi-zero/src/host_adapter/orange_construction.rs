use super::PiHostAdapter;
use crate::audio::AudioService;
use crate::main_paths::{default_samples_dir, default_store_dir};
use crate::platform_service::PiPlatformService;
use playback_runtime::UsbDataRole;
use std::path::{Path, PathBuf};
use std::sync::Arc;

impl PiHostAdapter {
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
        let audio_outputs = audio.audio_outputs();
        Ok(Self::with_platform_service_and_role(
            Some(audio),
            samples_dir,
            midi_in_handler,
            usb_midi_out_enabled,
            audio_outputs,
            platform_service,
            UsbDataRole::Gadget,
        ))
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
        let audio_outputs = audio.audio_outputs();
        Ok(Self::with_platform_service_and_role(
            Some(audio),
            samples_dir,
            midi_in_handler,
            usb_midi_out_enabled,
            audio_outputs,
            platform_service,
            UsbDataRole::Gadget,
        ))
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
