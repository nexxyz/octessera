use super::*;
use crate::render::HardwareRenderTargets;
use playback_runtime::{
    RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation, SyncSource,
};
use serde_json::json;
use std::sync::Arc;

struct Started {
    root: std::path::PathBuf,
    playback: PlaybackRuntime,
    runner: NativeRunner,
    adapter: PiHostAdapter,
}

fn start_with_saved_midi(usb_midi_out_enabled: bool, out_id: Option<&str>) -> Started {
    let root = crate::test_temp_dir::unique_temp_path("octessera-pi-startup-midi-error");
    let store = root.join("store");
    std::fs::create_dir_all(&store).unwrap();
    let mut payload: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../config/generated/pi/default.json"
    ))
    .unwrap();
    payload["runtimeConfig"]["midi"]["enabled"] = json!(true);
    payload["runtimeConfig"]["midi"]["clockOutEnabled"] = json!(true);
    payload["runtimeConfig"]["midi"]["outId"] = json!(out_id);
    payload["runtimeConfig"]["usb"]["midiOutEnabled"] = json!(usb_midi_out_enabled);
    let documents = playback_runtime::split_system_patch_documents(&payload).unwrap();
    std::fs::write(
        store.join("system.json"),
        serde_json::to_vec(&documents.system).unwrap(),
    )
    .unwrap();
    std::fs::write(
        store.join("default.patch.json"),
        serde_json::to_vec(&documents.patch).unwrap(),
    )
    .unwrap();
    let samples = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples")
        .canonicalize()
        .unwrap();
    let mut adapter = PiHostAdapter::new_with_data_role(
        Some(crate::audio::test_service_with_prep_worker()),
        store,
        samples,
        Arc::new(|_| {}),
        usb_midi_out_enabled,
        playback_runtime::AudioOutputSet::jack(),
        playback_runtime::UsbDataRole::Gadget,
    );
    adapter.set_test_midi_backend(
        vec!["MIDI Through Port-0".into()],
        vec!["MIDI Through Port-0".into()],
        [],
    );
    let (mut playback, mut runner) = init_runtime(AudioOptimization::Latency);
    runner.skip_startup_splash();
    initialize_host_state(&mut playback, &mut runner, &mut adapter).unwrap();
    crate::runtime_dispatch::dispatch_runtime_message(
        &mut playback,
        &mut runner,
        &mut adapter,
        HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        },
    )
    .unwrap();
    wait_for_initial_audio_prep(&mut playback, &mut runner, &mut adapter).unwrap();
    adapter.core.midi.sync_worker();
    crate::runtime_dispatch::handle_deferred_host_work(&mut playback, &mut runner, &mut adapter)
        .unwrap();
    Started {
        root,
        playback,
        runner,
        adapter,
    }
}

fn render_worker() -> RenderWorker {
    let (seesaw_tx, _seesaw_rx) = mpsc::channel();
    RenderWorker::spawn(HardwareRenderTargets {
        oled: crate::render::test_oled_output::fake_oled_output(),
        seesaw_tx,
        oled_handoff: None,
        hdmi: crate::render::hdmi::HdmiFramebuffer::new(),
    })
}

fn publish(started: &mut Started) -> Result<u64, String> {
    crate::runtime_init::publish_initial_scene(
        &started.playback,
        &mut started.runner,
        &mut started.adapter,
        &render_worker(),
    )
}

fn assert_starts_with_midi_error_frame(mut started: Started, message: &str) {
    let snapshot = started.playback.last_snapshot().unwrap();
    assert_eq!(snapshot["display"]["title"], "MENU");
    assert_eq!(snapshot["runtimeError"]["domain"], "midi");
    assert_eq!(snapshot["runtimeError"]["message"], message);

    assert!(publish(&mut started).unwrap() > 0);
    let _ = std::fs::remove_dir_all(&started.root);
}

#[test]
fn saved_usb_midi_without_the_boot_gadget_starts_with_a_midi_error_frame() {
    assert_starts_with_midi_error_frame(
        start_with_saved_midi(true, None),
        "USB MIDI gadget output not found",
    );
}

#[test]
fn unplugged_saved_midi_output_starts_without_an_error_and_stays_remembered() {
    let mut started = start_with_saved_midi(false, Some("name:Keyboard"));
    let snapshot = started.playback.last_snapshot().unwrap();
    assert!(snapshot["runtimeError"].is_null());
    assert_eq!(
        started.adapter.core.midi.selected_output_id().as_deref(),
        Some("name:Keyboard")
    );

    assert!(publish(&mut started).unwrap() > 0);
    let _ = std::fs::remove_dir_all(&started.root);
}

#[test]
fn non_midi_startup_error_still_blocks_the_initial_scene() {
    let mut started = start_with_saved_midi(false, None);
    started.playback.latch_facts(RuntimeErrorFacts::new(
        RuntimeErrorDomain::Storage,
        RuntimeErrorCode::OperationFailed,
        RuntimeOperation::StoreLoadSystem,
        Some("system load failed".into()),
    ));

    let error = publish(&mut started).unwrap_err();

    assert!(error.contains("system load failed"));
    let _ = std::fs::remove_dir_all(&started.root);
}
