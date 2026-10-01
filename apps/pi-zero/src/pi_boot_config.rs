use serde_json::Value;
use std::path::Path;

pub(crate) fn read_composed_runtime_config(
    store_dir: &Path,
) -> Result<Value, crate::usb_config::UsbConfigError> {
    let system_path = store_dir.join("system.json");
    let patch_path = store_dir.join("default.patch.json");
    let system = read_document(&system_path)?;
    let patch = read_document(&patch_path)?;
    playback_runtime::compose_local_system_patch_documents(&system, &patch).map_err(|error| {
        crate::usb_config::UsbConfigError::Invalid(format!(
            "invalid System/Patch documents: {error}"
        ))
    })
}

fn read_document(path: &Path) -> Result<Value, crate::usb_config::UsbConfigError> {
    let display = path.display().to_string();
    let content =
        std::fs::read_to_string(path).map_err(|error| crate::usb_config::UsbConfigError::Read {
            path: display.clone(),
            message: error.to_string(),
        })?;
    serde_json::from_str(&content).map_err(|error| crate::usb_config::UsbConfigError::Parse {
        path: display,
        message: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use playback_runtime::{
        compose_system_patch_documents, split_local_system_patch_documents,
        split_system_patch_documents,
    };
    use std::path::PathBuf;

    fn store() -> PathBuf {
        std::env::temp_dir().join(format!(
            "octessera-pi-boot-config-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn full_default() -> Value {
        serde_json::from_str(include_str!("../../../config/generated/pi/default.json")).unwrap()
    }

    fn write_pair(store: &Path, system: &Value, patch: &Value) {
        std::fs::create_dir_all(store).unwrap();
        std::fs::write(
            store.join("system.json"),
            serde_json::to_vec(system).unwrap(),
        )
        .unwrap();
        std::fs::write(
            store.join("default.patch.json"),
            serde_json::to_vec(patch).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn composed_pi_boot_pair_preserves_system_and_patch_values() {
        let root = store();
        let full = full_default();
        let pair = split_system_patch_documents(&full).unwrap();
        write_pair(&root, &pair.system, &pair.patch);

        assert_eq!(
            read_composed_runtime_config(&root).unwrap(),
            compose_system_patch_documents(&pair.system, &pair.patch).unwrap()
        );
        assert_eq!(
            std::fs::read(root.join("system.json")).unwrap(),
            serde_json::to_vec(&pair.system).unwrap()
        );
        assert_eq!(
            std::fs::read(root.join("default.patch.json")).unwrap(),
            serde_json::to_vec(&pair.patch).unwrap()
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn composed_boot_pair_keeps_system_routes_and_local_sample_ids() {
        let root = store();
        let mut full = full_default();
        full["runtimeConfig"]["displayBrightness"] = serde_json::json!(41);
        full["runtimeConfig"]["audioOutputs"]["dac"] = serde_json::json!(true);
        full["runtimeConfig"]["audioOutputs"]["hdmi"] = serde_json::json!(true);
        full["runtimeConfig"]["audioOutputs"]["usb"] = serde_json::json!(false);
        full["runtimeConfig"]["usb"]["dataRole"] = serde_json::json!("host");
        full["runtimeConfig"]["sound"]["optimizeFor"] = serde_json::json!("capacity");
        full["runtimeConfig"]["instruments"][0]["type"] = serde_json::json!("sampler");
        full["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"] =
            serde_json::json!("userdata/User Kit/custom.wav");
        full["runtimeConfig"]["instruments"][0]["sample"]["slots"][4]["path"] =
            serde_json::json!("sd-card/octessera/samples/kick.wav");
        full["runtimeConfig"]["instruments"][0]["sample"]["assignments"] =
            serde_json::json!([{ "level": null, "sampleSlot": 0, "x": 0, "y": 0 }]);
        let pair = split_local_system_patch_documents(&full).unwrap();
        write_pair(&root, &pair.system, &pair.patch);

        let prepared = read_composed_runtime_config(&root).unwrap();
        assert_eq!(
            prepared["runtimeConfig"]["displayBrightness"],
            serde_json::json!(41)
        );
        assert_eq!(prepared["runtimeConfig"]["audioOutputs"]["dac"], true);
        assert_eq!(prepared["runtimeConfig"]["audioOutputs"]["hdmi"], true);
        assert_eq!(prepared["runtimeConfig"]["audioOutputs"]["usb"], false);
        assert_eq!(
            prepared["runtimeConfig"]["usb"]["dataRole"],
            serde_json::json!("host")
        );
        assert_eq!(
            prepared["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"],
            "userdata/User Kit/custom.wav"
        );
        assert_eq!(
            prepared["runtimeConfig"]["instruments"][0]["sample"]["slots"][4]["path"],
            "sd-card/octessera/samples/kick.wav"
        );
        let (usb, audio_optimization) = crate::usb_config::read_boot_runtime_config(&root).unwrap();
        assert_eq!(usb.data_role, playback_runtime::UsbDataRole::Host);
        assert!(usb.audio_outputs.dac());
        assert!(usb.audio_outputs.hdmi());
        assert!(!usb.audio_outputs.usb());
        assert_eq!(
            audio_optimization,
            playback_runtime::AudioOptimization::Capacity
        );
        assert!(compose_system_patch_documents(&pair.system, &pair.patch).is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn unsafe_local_sample_path_fails_closed_without_rewriting_the_pair() {
        let root = store();
        let pair = split_local_system_patch_documents(&full_default()).unwrap();
        let mut unsafe_patch = pair.patch.clone();
        unsafe_patch["runtimeConfig"]["instruments"][0]["type"] = serde_json::json!("sampler");
        unsafe_patch["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"] =
            serde_json::json!("../escape.wav");
        write_pair(&root, &pair.system, &unsafe_patch);
        let system_bytes = std::fs::read(root.join("system.json")).unwrap();
        let patch_bytes = std::fs::read(root.join("default.patch.json")).unwrap();

        assert!(read_composed_runtime_config(&root).is_err());
        assert_eq!(
            std::fs::read(root.join("system.json")).unwrap(),
            system_bytes
        );
        assert_eq!(
            std::fs::read(root.join("default.patch.json")).unwrap(),
            patch_bytes
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn mixed_partial_corrupt_and_conflicting_stores_fail_without_repair() {
        let root = store();
        std::fs::create_dir_all(&root).unwrap();
        let legacy = serde_json::to_vec(&full_default()).unwrap();
        std::fs::write(root.join("default.json"), &legacy).unwrap();
        assert!(read_composed_runtime_config(&root).is_err());
        assert_eq!(std::fs::read(root.join("default.json")).unwrap(), legacy);

        let full = full_default();
        let pair = split_system_patch_documents(&full).unwrap();
        write_pair(&root, &pair.system, &pair.patch);
        std::fs::remove_file(root.join("default.patch.json")).unwrap();
        assert!(read_composed_runtime_config(&root).is_err());
        assert!(root.join("system.json").is_file());

        std::fs::write(root.join("default.patch.json"), b"{").unwrap();
        assert!(read_composed_runtime_config(&root).is_err());
        assert_eq!(
            std::fs::read(root.join("default.patch.json")).unwrap(),
            b"{"
        );

        let mut conflicting = pair.system;
        conflicting["runtimeConfig"]["bpm"] = serde_json::json!(99);
        write_pair(&root, &conflicting, &pair.patch);
        let system_bytes = std::fs::read(root.join("system.json")).unwrap();
        let patch_bytes = std::fs::read(root.join("default.patch.json")).unwrap();
        assert!(read_composed_runtime_config(&root).is_err());
        assert_eq!(
            std::fs::read(root.join("system.json")).unwrap(),
            system_bytes
        );
        assert_eq!(
            std::fs::read(root.join("default.patch.json")).unwrap(),
            patch_bytes
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
