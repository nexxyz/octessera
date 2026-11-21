use super::*;
use crate::usb_config::UsbAudioOut;
use playback_runtime::{
    HostAdapter, NativeManualSaveRequest, NativeRunnerConfig, RuntimeConfig, RuntimeOperation,
    RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult,
};
use serde_json::json;
use std::sync::Arc;

struct Rig {
    adapter: PiHostAdapter,
    runner: NativeRunner,
    playback: PlaybackRuntime,
    root: std::path::PathBuf,
}

fn rig(auto_save: bool, rolling_backups: bool) -> Rig {
    let root = crate::test_temp_dir::unique_temp_path("octessera-pi-stale-autosave");
    let store = root.join("store");
    std::fs::create_dir_all(&store).unwrap();
    let mut full: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../config/generated/pi/default.json"
    ))
    .unwrap();
    full["runtimeConfig"]["autoSaveDefault"] = json!(auto_save);
    full["runtimeConfig"]["rollingBackups"] = json!(rolling_backups);
    let documents = playback_runtime::split_system_patch_documents(&full).unwrap();
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
    let mut adapter = PiHostAdapter::new(
        None,
        store,
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        UsbAudioOut::Jack,
    );
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    crate::runtime_output::initialize_host_state(&mut playback, &mut runner, &mut adapter).unwrap();
    Rig {
        adapter,
        runner,
        playback,
        root,
    }
}

fn press(runner: &mut NativeRunner, inputs: &[serde_json::Value]) {
    for input in inputs {
        runner
            .send_music_first(HostMessage::DeviceInput {
                input: input.clone(),
                request_snapshot: Some(false),
            })
            .unwrap();
    }
}

fn edit(rig: &mut Rig) {
    rig.runner
        .test_focus_menu_item("aux:0:turn.instruments.0.synth.osc1.levelPct")
        .unwrap();
    press(
        &mut rig.runner,
        &[
            json!({"type":"encoder_press","id":"main"}),
            json!({"type":"button_s","pressed":true}),
            json!({"type":"button_s","pressed":false}),
            json!({"type":"encoder_turn","id":"aux1","delta":-1}),
        ],
    );
    std::thread::sleep(Duration::from_millis(160));
}

fn flush(rig: &mut Rig, now: Instant) {
    let messages = rig.adapter.core.flush_native_persistence_at(
        &mut rig.playback,
        &mut rig.runner,
        now,
        false,
    );
    assert!(messages.is_empty());
}

fn finished_operation(rig: &mut Rig, operation: RuntimeOperation) -> Vec<HostMessage> {
    for _ in 0..1000 {
        let messages = rig
            .adapter
            .core
            .finished_platform_results(&mut rig.runner, 8);
        if messages.iter().any(|message| {
            matches!(message, HostMessage::RuntimeResult { result } if result.operation() == operation)
        }) {
            return messages;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("{operation:?} did not finish");
}

fn finish_save(rig: &mut Rig) {
    for message in finished_operation(rig, RuntimeOperation::StoreSavePreset) {
        rig.playback
            .dispatch_host_message_music_first(message, &mut rig.runner, &mut rig.adapter)
            .unwrap();
    }
}

fn save_as_mine(rig: &mut Rig) {
    let manual = NativeManualSaveRequest::Preset {
        name: "Mine".into(),
        mode: None,
        rename_from: None,
    };
    let refused = crate::platform_service::platform_native_persistence::submit_manual_save(
        &rig.adapter.core.platform_service,
        &mut rig.playback,
        &mut rig.runner,
        manual,
    );
    assert!(refused.is_none());
    finish_save(rig);
}

fn load_mine(rig: &mut Rig) -> Vec<HostMessage> {
    rig.adapter
        .handle_platform_effect(&RuntimePlatformRequest::new(
            RuntimePlatformEffect::StoreLoadPreset {
                name: "Mine".into(),
            },
            "load-mine".into(),
            Some(9),
        ))
        .unwrap()
}

fn assert_loaded(messages: &[HostMessage]) {
    assert!(
        matches!(
            messages,
            [HostMessage::RuntimeResult {
                result: RuntimeStoreResult::LoadPresetResult {
                    payload: Some(_),
                    ..
                }
            }]
        ),
        "the load was not accepted: {messages:?}"
    );
}

#[test]
fn booting_a_factory_store_leaves_nothing_to_autosave() {
    let rig = rig(true, false);

    assert!(!rig.runner.owes_patch_persistence());
    assert!(rig
        .runner
        .persistence_intent_at(Instant::now() + Duration::from_secs(5))
        .is_none());
    let _ = std::fs::remove_dir_all(rig.root);
}

#[test]
fn a_preset_save_that_cleans_the_patch_leaves_no_autosave_to_refuse_the_next_load() {
    let mut rig = rig(true, false);
    edit(&mut rig);
    let now = Instant::now();
    flush(&mut rig, now);
    assert!(rig.adapter.core.pending_default_save.has_default_pending());

    save_as_mine(&mut rig);
    assert!(!rig.runner.owes_patch_persistence());
    flush(&mut rig, now + Duration::from_millis(10));

    assert!(!rig.adapter.core.pending_default_save.has_default_pending());
    assert_loaded(&load_mine(&mut rig));
    let _ = std::fs::remove_dir_all(rig.root);
}

#[test]
fn turning_auto_save_off_drops_the_autosave_it_queued() {
    let mut rig = rig(true, false);
    save_as_mine(&mut rig);
    edit(&mut rig);
    let now = Instant::now();
    flush(&mut rig, now);
    assert!(rig.adapter.core.pending_default_save.has_default_pending());

    rig.runner.test_focus_menu_item("autoSaveDefault").unwrap();
    press(
        &mut rig.runner,
        &[
            json!({"type":"encoder_press","id":"main"}),
            json!({"type":"encoder_turn","id":"main","delta":-1}),
            json!({"type":"encoder_press","id":"main"}),
        ],
    );
    assert!(!rig.runner.owes_patch_persistence());
    flush(&mut rig, now + Duration::from_millis(10));

    assert!(!rig.adapter.core.pending_default_save.has_default_pending());
    assert_loaded(&load_mine(&mut rig));
    let _ = std::fs::remove_dir_all(rig.root);
}

#[test]
fn a_backup_alone_is_still_written_when_auto_save_is_off() {
    let mut rig = rig(false, true);
    edit(&mut rig);
    assert!(rig.runner.owes_patch_persistence());
    flush(&mut rig, Instant::now());

    finished_operation(&mut rig, RuntimeOperation::StoreSaveBackup);
    let _ = std::fs::remove_dir_all(rig.root);
}
