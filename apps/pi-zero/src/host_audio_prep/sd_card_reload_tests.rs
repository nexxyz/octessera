use super::owner_prep::process_instrument_slot;
use playback_runtime::{
    CoreRunner, HostMessage, NativeRunner, NativeRunnerConfig, RunnerMessage, RuntimeAudioCommand,
    RuntimeStoreResult,
};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::mpsc;

const SNARE: &str = "sd-card/octessera/samples/snare.wav";

fn slot_configs(messages: &[RunnerMessage]) -> Vec<Value> {
    messages
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::AudioCommands { commands } => commands.as_slice(),
            _ => &[],
        })
        .filter_map(|command| match command {
            RuntimeAudioCommand::SetInstrumentSlot {
                instrument_slot: 0,
                config,
                ..
            } => Some(config.clone()),
            RuntimeAudioCommand::SetAudioConfig { config, .. } => {
                Some(config["instruments"][0].clone())
            }
            _ => None,
        })
        .collect()
}

fn prepare_and_report(
    runner: &mut NativeRunner,
    audio: &crate::audio::AudioService,
    root: &Path,
    messages: &[RunnerMessage],
) {
    let configs = slot_configs(messages);
    assert!(!configs.is_empty(), "no audio load for I1: {messages:?}");
    let (result_tx, result_rx) = mpsc::channel();
    for config in configs {
        process_instrument_slot(audio, 1, 0, 1, config, root.to_path_buf(), &result_tx);
    }
    drop(result_tx);
    for result in result_rx {
        runner.send(result).unwrap();
    }
}

fn sample_row(runner: &mut NativeRunner) -> String {
    runner
        .test_focus_menu_item(&format!("sample.loaded:0:0:{SNARE}"))
        .unwrap()
}

#[test]
fn a_card_sample_missing_at_load_plays_once_the_card_mounts() {
    let root = crate::test_temp_dir::unique_temp_path("octessera-pi-sd2-reload");
    std::fs::create_dir_all(root.join("sd-card/octessera/samples")).unwrap();
    let (audio, _, _event_rx, _) =
        crate::audio::test_service_with_recording_dir(root.join("recordings"));
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    let _ = runner.messages_with_snapshot().unwrap();
    let mut payload = runner.test_config_payload();
    payload["runtimeConfig"]["instruments"][0]["type"] = json!("sampler");
    payload["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"] = json!(SNARE);
    runner.apply_config_payload(payload).unwrap();
    let loaded = runner.messages_with_snapshot().unwrap();

    prepare_and_report(&mut runner, &audio, &root, &loaded);
    let row = sample_row(&mut runner);
    assert!(row.contains("N/A-"), "{row}");
    assert_eq!(runner.test_display_title(), "Samples missing");

    std::fs::write(root.join(SNARE), super::tests::owner_wav_bytes()).unwrap();
    let mounted = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SdCardMounted,
        })
        .unwrap();
    prepare_and_report(&mut runner, &audio, &root, &mounted);

    let row = sample_row(&mut runner);
    assert!(!row.contains("N/A-"), "{row}");
    assert_ne!(runner.test_display_title(), "Samples missing");
    let _ = std::fs::remove_dir_all(root);
}
