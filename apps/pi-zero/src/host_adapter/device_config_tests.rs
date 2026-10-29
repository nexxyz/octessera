use super::*;
use crate::device_apply::{UsbChange, APPLY_FAILED_MESSAGE};
use std::sync::Mutex;
use std::time::Duration;

type Calls = Arc<Mutex<Vec<UsbChange>>>;

fn system(role: UsbDataRole, usb_audio: bool, usb_midi: bool) -> serde_json::Value {
    let mut full = crate::user_data_archive::canonical_defaults();
    full["runtimeConfig"]["usb"]["dataRole"] = serde_json::json!(role.as_str());
    full["runtimeConfig"]["audioOutputs"]["usb"] = serde_json::json!(usb_audio);
    full["runtimeConfig"]["usb"]["midiOutEnabled"] = serde_json::json!(usb_midi);
    playback_runtime::split_system_patch_documents(&full)
        .unwrap()
        .system
}

fn adapter(
    label: &str,
    result: Result<(), String>,
) -> (PiHostAdapter, AudioService, Calls, std::path::PathBuf) {
    let root = crate::test_temp_dir::unique_temp_path(label);
    crate::pi_store_test_support::write_pair(
        &root.join("store"),
        &crate::user_data_archive::canonical_defaults(),
    );
    let calls = Calls::default();
    let observed = calls.clone();
    let service = PiPlatformService::new_with_usb_applier(
        root.join("store"),
        root.join("samples"),
        Arc::new(move |change| {
            observed.lock().unwrap().push(change);
            result.clone()
        }),
    );
    let audio = crate::audio::test_service_for_sample_prep();
    let mut adapter = PiHostAdapter::with_platform_service_and_role(
        Some(audio.clone()),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        AudioOutputSet::jack(),
        service,
        UsbDataRole::Gadget,
    );
    adapter.set_test_midi_backend(Vec::new(), Vec::new(), []);
    (adapter, audio, calls, root)
}

fn apply(adapter: &mut PiHostAdapter, payload: &serde_json::Value) -> RuntimeStoreResult {
    let messages = adapter
        .handle_platform_effect(&RuntimePlatformRequest::new(
            RuntimePlatformEffect::ApplyDeviceConfig {
                payload: payload.clone(),
            },
            "apply".into(),
            None,
        ))
        .unwrap();
    assert!(messages.is_empty(), "{messages:?}");
    adapter
        .core
        .platform_service
        .enqueue_test_barrier()
        .unwrap()
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    match adapter.core.platform_service.drain_results(4).as_slice() {
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { result, .. },
        }] => result.as_ref().clone(),
        results => panic!("expected one device apply result: {results:?}"),
    }
}

fn status(ok: bool, message: &str) -> RuntimeStoreResult {
    RuntimeStoreResult::DeviceConfigStatus {
        ok,
        message: message.into(),
    }
}

#[test]
fn usb_audio_and_midi_apply_live_and_rebuild_the_gadget_once_each() {
    let (mut adapter, audio, calls, root) = adapter("octessera-device-apply-gadget", Ok(()));
    let usb_audio = system(UsbDataRole::Gadget, true, false);

    assert_eq!(apply(&mut adapter, &usb_audio), status(true, "USB ready"));
    assert!(audio.audio_outputs().usb());
    assert_eq!(
        crate::platform_service::load_json(&root.join("store/system.json")).unwrap(),
        Some(usb_audio.clone())
    );

    let mut hdmi = usb_audio;
    hdmi["runtimeConfig"]["audioOutputs"]["hdmi"] = serde_json::json!(true);
    assert_eq!(apply(&mut adapter, &hdmi), status(true, "Applied"));
    assert!(audio.audio_outputs().hdmi());

    let usb_midi = system(UsbDataRole::Gadget, false, true);
    assert_eq!(apply(&mut adapter, &usb_midi), status(true, "USB ready"));
    assert!(!audio.audio_outputs().usb());
    assert!(!audio.audio_outputs().hdmi());
    assert_eq!(
        *calls.lock().unwrap(),
        vec![UsbChange::Gadget, UsbChange::Gadget]
    );
    assert_eq!(
        usb_sd_transfer_output_block_reason(
            adapter.audio_outputs.usb(),
            adapter.usb_midi_out_enabled
        ),
        Some("USB SD2 transfer blocked while USB MIDI out is enabled")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_failed_usb_step_keeps_the_saved_system_for_the_next_boot() {
    let (mut adapter, _audio, _calls, root) =
        adapter("octessera-device-apply-failed", Err("helper failed".into()));
    let usb_midi = system(UsbDataRole::Gadget, false, true);

    assert_eq!(
        apply(&mut adapter, &usb_midi),
        status(false, APPLY_FAILED_MESSAGE)
    );
    assert_eq!(
        crate::platform_service::load_json(&root.join("store/system.json")).unwrap(),
        Some(usb_midi)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[test]
fn usb_role_switches_live_and_follows_the_keyboard_and_sd2_rules() {
    let (mut adapter, _audio, calls, root) = adapter("octessera-device-apply-role", Ok(()));
    let control = crate::keyboard_capture::KeyboardCaptureControl::new(false);
    adapter.core.set_keyboard_capture_control(control.clone());

    let host = system(UsbDataRole::Host, false, false);
    assert_eq!(apply(&mut adapter, &host), status(true, "USB ready"));
    assert_eq!(adapter.usb_data_role, UsbDataRole::Host);
    assert!(control.buses().usb);
    let sd2 = adapter
        .handle_platform_effect(&RuntimePlatformRequest::new(
            RuntimePlatformEffect::UsbSdTransferStart,
            "host-sd2".into(),
            None,
        ))
        .unwrap();
    assert!(matches!(
        sd2.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure { .. }
        }]
    ));

    let gadget = system(UsbDataRole::Gadget, false, false);
    assert_eq!(apply(&mut adapter, &gadget), status(true, "USB ready"));
    assert!(!control.buses().usb);
    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            UsbChange::Role(UsbDataRole::Host),
            UsbChange::Role(UsbDataRole::Gadget)
        ]
    );
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[test]
fn host_role_is_refused_before_saving_while_sd2_transfer_is_active() {
    let (mut adapter, _audio, calls, root) = adapter("octessera-device-apply-sd2", Ok(()));
    let prior = std::fs::read(root.join("store/system.json")).unwrap();
    std::fs::write(root.join("storage.state"), b"active").unwrap();

    assert_eq!(
        apply(&mut adapter, &system(UsbDataRole::Host, false, false)),
        status(false, "Stop SD2 transfer first")
    );
    assert_eq!(
        std::fs::read(root.join("store/system.json")).unwrap(),
        prior
    );
    assert!(calls.lock().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn host_role_payload_with_usb_outputs_is_rejected_without_changes() {
    let (mut adapter, audio, calls, root) = adapter("octessera-device-apply-invalid", Ok(()));
    let mut invalid = system(UsbDataRole::Host, false, false);
    invalid["runtimeConfig"]["audioOutputs"]["usb"] = serde_json::json!(true);
    let messages = adapter
        .handle_platform_effect(&RuntimePlatformRequest::new(
            RuntimePlatformEffect::ApplyDeviceConfig { payload: invalid },
            "invalid".into(),
            None,
        ))
        .unwrap();
    assert!(matches!(
        messages.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure { .. }
        }]
    ));
    assert!(!audio.audio_outputs().usb());
    assert_eq!(adapter.usb_data_role, UsbDataRole::Gadget);
    assert!(calls.lock().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(root);
}
