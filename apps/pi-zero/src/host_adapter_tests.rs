use super::*;

#[test]
fn preset_path_rejects_unsafe_names() {
    let adapter = PiPlaybackHostAdapter::new(
        None,
        PathBuf::from("store"),
        PathBuf::from("samples"),
        Arc::new(|_| {}),
        false,
        UsbAudioOut::Jack,
    );
    assert!(crate::platform_service::preset_path(&adapter.store_dir, "safe").is_ok());
    for name in ["bad/name", r"bad\name", r"C:\x", "CON", "bad:name"] {
        assert!(
            crate::platform_service::preset_path(&adapter.store_dir, name).is_err(),
            "{name:?}"
        );
    }
}
