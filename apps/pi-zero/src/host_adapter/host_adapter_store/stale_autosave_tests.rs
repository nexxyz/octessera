use super::*;
use crate::usb_config::UsbAudioOut;
use playback_runtime::{
    HostAdapter, NativeManualSaveRequest, NativeRunnerConfig, RuntimeConfig, RuntimeOperation,
    RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult,
};
use serde_json::json;
use std::sync::Arc;

pub(super) struct Rig {
    pub(super) adapter: PiHostAdapter,
    pub(super) runner: NativeRunner,
    pub(super) playback: PlaybackRuntime,
    pub(super) root: std::path::PathBuf,
}

pub(super) fn rig(auto_save: bool, rolling_backups: bool) -> Rig {
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
    let mut rig = Rig {
        adapter,
        runner,
        playback,
        root,
    };
    dismiss_latched_errors(&mut rig);
    rig
}

/// A host without MIDI latches a runtime error that takes the next presses, so Back clears it first.
fn dismiss_latched_errors(rig: &mut Rig) {
    for _ in 0..10 {
        crate::runtime_dispatch::handle_deferred_host_work(
            &mut rig.playback,
            &mut rig.runner,
            &mut rig.adapter,
        )
        .unwrap();
        if rig.playback.latched_errors().is_empty() {
            break;
        }
        for message in [
            crate::input::neokey_message(0, true).unwrap(),
            crate::input::neokey_message(0, false).unwrap(),
        ] {
            crate::runtime_dispatch::dispatch(
                &mut rig.playback,
                &mut rig.runner,
                &mut rig.adapter,
                message,
            )
            .unwrap();
        }
    }
    assert!(rig.playback.latched_errors().is_empty());
}

pub(super) fn press(runner: &mut NativeRunner, inputs: &[serde_json::Value]) {
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

fn dispatch_all(rig: &mut Rig, messages: Vec<HostMessage>) {
    for message in messages {
        rig.playback
            .dispatch_host_message_music_first(message, &mut rig.runner, &mut rig.adapter)
            .unwrap();
    }
}

fn force_flush(rig: &mut Rig) {
    let messages = rig.adapter.core.flush_native_persistence_at(
        &mut rig.playback,
        &mut rig.runner,
        Instant::now(),
        true,
    );
    dispatch_all(rig, messages);
}

fn finish_default_save(rig: &mut Rig) {
    let messages = finished_operation(rig, RuntimeOperation::StoreSaveDefault);
    dispatch_all(rig, messages);
}

pub(super) fn finish_save(rig: &mut Rig) {
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
    load_preset(rig, "Mine")
}

fn load_preset(rig: &mut Rig, name: &str) -> Vec<HostMessage> {
    rig.adapter
        .handle_platform_effect(&RuntimePlatformRequest::new(
            RuntimePlatformEffect::StoreLoadPreset { name: name.into() },
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

    assert!(!rig.runner.owes_default_autosave());
    assert!(rig
        .runner
        .persistence_intent_at(Instant::now() + Duration::from_secs(5))
        .is_none());
    let _ = std::fs::remove_dir_all(rig.root);
}

#[test]
fn a_preset_save_under_auto_save_also_writes_the_default_patch() {
    let mut rig = rig(true, false);
    let factory_level = saved_level(&rig, "default.patch.json");
    edit(&mut rig);
    let now = Instant::now();
    flush(&mut rig, now);
    assert!(rig.adapter.core.pending_default_save.has_default_pending());

    rig.runner
        .test_focus_menu_item("preset.saveAs.save")
        .unwrap();
    press(
        &mut rig.runner,
        &[
            json!({"type":"encoder_press","id":"main"}),
            json!({"type":"encoder_turn","id":"main","delta":1}),
            json!({"type":"encoder_press","id":"main"}),
        ],
    );
    let refused = rig
        .adapter
        .core
        .take_manual_save(&mut rig.playback, &mut rig.runner);
    assert!(refused.is_none());
    let saved = finished_operation(&mut rig, RuntimeOperation::StoreSavePreset);
    dispatch_all(&mut rig, saved);
    assert!(rig.runner.owes_default_autosave());
    flush(&mut rig, now + Duration::from_millis(10));
    flush(&mut rig, now + Duration::from_millis(2500));
    finish_default_save(&mut rig);

    let name = std::fs::read_dir(rig.root.join("store/patches"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .file_name()
        .to_string_lossy()
        .trim_end_matches(".json")
        .to_string();
    let preset_level = saved_level(&rig, &format!("patches/{name}.json"));
    assert_ne!(preset_level, factory_level);
    assert_eq!(saved_level(&rig, "default.patch.json"), preset_level);
    assert!(!rig.runner.owes_default_autosave());
    assert!(!rig.adapter.core.pending_default_save.has_default_pending());
    assert_loaded(&load_preset(&mut rig, &name));
    let _ = std::fs::remove_dir_all(rig.root);
}

#[test]
fn turning_auto_save_on_after_a_load_leaves_no_stuck_autosave() {
    let mut rig = rig(false, false);
    save_as_mine(&mut rig);
    edit(&mut rig);
    for message in load_mine(&mut rig) {
        rig.playback
            .dispatch_host_message_music_first(message, &mut rig.runner, &mut rig.adapter)
            .unwrap();
    }
    force_flush(&mut rig);
    assert!(!rig.adapter.core.pending_default_save.has_default_pending());

    rig.runner.test_focus_menu_item("autoSaveDefault").unwrap();
    press(
        &mut rig.runner,
        &[
            json!({"type":"encoder_press","id":"main"}),
            json!({"type":"encoder_turn","id":"main","delta":1}),
            json!({"type":"encoder_press","id":"main"}),
        ],
    );
    force_flush(&mut rig);

    assert!(!rig.adapter.core.pending_default_save.has_default_pending());
    assert_loaded(&load_mine(&mut rig));
    let _ = std::fs::remove_dir_all(rig.root);
}

#[test]
fn turning_auto_save_off_drops_the_autosave_it_queued() {
    // Backups on, as shipped: the edit's flush also issues the backup, so afterwards the runner
    // owes nothing that would rebuild the queued record.
    let mut rig = rig(true, true);
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
    assert!(!rig.runner.owes_default_autosave());
    flush(&mut rig, now + Duration::from_millis(10));

    assert!(!rig.adapter.core.pending_default_save.has_default_pending());
    assert_loaded(&load_mine(&mut rig));
    let _ = std::fs::remove_dir_all(rig.root);
}

#[test]
fn a_backup_alone_is_still_written_when_auto_save_is_off() {
    let mut rig = rig(false, true);
    edit(&mut rig);
    assert!(!rig.runner.owes_default_autosave());
    flush(&mut rig, Instant::now());

    finished_operation(&mut rig, RuntimeOperation::StoreSaveBackup);
    let _ = std::fs::remove_dir_all(rig.root);
}

fn saved_level(rig: &Rig, file: &str) -> serde_json::Value {
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(rig.root.join("store").join(file)).unwrap()).unwrap();
    saved["runtimeConfig"]["instruments"][0]["synth"]["osc1"]["levelPct"].clone()
}

fn load_mine_and_flush(rig: &mut Rig) {
    for message in load_mine(rig) {
        rig.playback
            .dispatch_host_message_music_first(message, &mut rig.runner, &mut rig.adapter)
            .unwrap();
    }
    std::thread::sleep(Duration::from_millis(160));
    let now = Instant::now();
    flush(rig, now);
    force_flush(rig);
}

#[test]
fn with_auto_save_on_a_loaded_preset_becomes_the_saved_default_patch() {
    let mut rig = rig(true, false);
    let factory_level = saved_level(&rig, "default.patch.json");
    edit(&mut rig);
    save_as_mine(&mut rig);
    let preset_level = saved_level(&rig, "patches/Mine.json");
    assert_ne!(preset_level, factory_level);
    assert_eq!(saved_level(&rig, "default.patch.json"), factory_level);

    load_mine_and_flush(&mut rig);
    finished_operation(&mut rig, RuntimeOperation::StoreSaveDefault);

    assert_eq!(saved_level(&rig, "default.patch.json"), preset_level);
    let _ = std::fs::remove_dir_all(rig.root);
}

#[test]
fn with_auto_save_off_a_loaded_preset_writes_no_default_patch() {
    let mut rig = rig(false, false);
    let factory_level = saved_level(&rig, "default.patch.json");
    edit(&mut rig);
    save_as_mine(&mut rig);

    load_mine_and_flush(&mut rig);
    std::thread::sleep(Duration::from_millis(100));

    assert!(rig
        .adapter
        .core
        .finished_platform_results(&mut rig.runner, 8)
        .iter()
        .all(
            |message| !matches!(message, HostMessage::RuntimeResult { result }
            if result.operation() == RuntimeOperation::StoreSaveDefault)
        ));
    assert_eq!(saved_level(&rig, "default.patch.json"), factory_level);
    let _ = std::fs::remove_dir_all(rig.root);
}
