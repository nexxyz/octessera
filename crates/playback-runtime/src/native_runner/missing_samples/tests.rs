use super::super::{InstrumentType, NativeRunnerConfig, NativeSampleAvailability};
use super::*;
use crate::{CoreRunner, HostMessage, RunnerMessage, RuntimeAudioCommand, RuntimeStoreResult};
use std::time::{Duration, Instant};

fn runner_with_two_samples() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[1].kind = InstrumentType::Sampler;
    runner.instruments[1].sample_paths[0] = Some("samples/Drum/kick/Kick2.wav".into());
    runner.instruments[1].sample_paths[1] = Some("userdata/User Kit/gone.wav".into());
    runner
}

fn report_missing(runner: &mut NativeRunner, instrument_slot: Option<usize>, paths: &[&str]) {
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SamplesMissing {
                instrument_slot,
                paths: paths.iter().map(|path| path.to_string()).collect(),
            },
        })
        .unwrap();
}

#[test]
fn reported_missing_sample_marks_only_its_slot_and_lists_it_in_a_popup() {
    let mut runner = runner_with_two_samples();
    report_missing(&mut runner, None, &["userdata/User Kit/gone.wav"]);

    assert_eq!(
        runner.sample_availability[1][1],
        NativeSampleAvailability::Unavailable
    );
    assert_ne!(
        runner.sample_availability[1][0],
        NativeSampleAvailability::Unavailable
    );
    let popup = runner
        .display
        .help_popup
        .as_ref()
        .expect("missing samples popup");
    assert_eq!(popup.title, "Samples missing");
    assert!(
        popup.lines.join(" ").contains("gone.wav"),
        "{:?}",
        popup.lines
    );
    assert!(runner.display.runtime_error_presentation.is_none());
}

#[test]
fn already_reported_missing_samples_do_not_reopen_the_popup() {
    let mut runner = runner_with_two_samples();
    report_missing(&mut runner, None, &["userdata/User Kit/gone.wav"]);
    runner.display.help_popup = None;

    report_missing(&mut runner, None, &["userdata/User Kit/gone.wav"]);

    assert!(runner.display.help_popup.is_none());
}

#[test]
fn unrelated_missing_paths_are_ignored() {
    let mut runner = runner_with_two_samples();
    report_missing(&mut runner, None, &["samples/Drum/kick/elsewhere.wav"]);

    assert!(runner.display.help_popup.is_none());
    assert!(runner.sample_availability[1]
        .iter()
        .all(|availability| *availability != NativeSampleAvailability::Unavailable));
}

const SD_CARD_SNARE: &str = "sd-card/octessera/samples/snare.wav";

fn runner_with_an_sd_card_sample() -> NativeRunner {
    let mut runner = runner_with_two_samples();
    runner.instruments[1].sample_paths[2] = Some(SD_CARD_SNARE.into());
    let _ = runner.messages_with_snapshot().unwrap();
    runner
}

fn report_loaded(runner: &mut NativeRunner, instrument_slot: usize, paths: &[&str]) {
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SamplesLoaded {
                instrument_slot,
                paths: paths.iter().map(|path| path.to_string()).collect(),
            },
        })
        .unwrap();
}

fn sd_card_mounted(runner: &mut NativeRunner) -> Vec<usize> {
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SdCardMounted,
        })
        .unwrap()
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::AudioCommands { commands } => commands.as_slice(),
            _ => &[],
        })
        .filter_map(|command| match command {
            RuntimeAudioCommand::SetInstrumentSlot {
                instrument_slot, ..
            } => Some(*instrument_slot),
            _ => None,
        })
        .collect()
}

fn sd_card_slot(runner: &NativeRunner) -> NativeSampleAvailability {
    runner.sample_availability[1][2]
}

#[test]
fn a_patch_loaded_without_sd2_loads_its_card_samples_once_the_card_mounts() {
    let mut runner = runner_with_an_sd_card_sample();
    report_missing(&mut runner, None, &[SD_CARD_SNARE]);
    assert_eq!(sd_card_slot(&runner), NativeSampleAvailability::Unavailable);

    assert_eq!(sd_card_mounted(&mut runner), vec![1]);
    assert_eq!(sd_card_slot(&runner), NativeSampleAvailability::Unavailable);
    assert!(runner.display.help_popup.is_some());

    report_loaded(&mut runner, 1, &[SD_CARD_SNARE]);
    assert_eq!(sd_card_slot(&runner), NativeSampleAvailability::Available);
    assert!(runner.display.help_popup.is_none());
}

#[test]
fn a_mount_leaves_card_samples_that_already_play_alone() {
    let mut runner = runner_with_an_sd_card_sample();

    assert!(sd_card_mounted(&mut runner).is_empty());
    assert_ne!(sd_card_slot(&runner), NativeSampleAvailability::Unavailable);
}

#[test]
fn a_swap_to_a_card_without_the_file_keeps_its_slots_unavailable_without_reopening_the_popup() {
    let mut runner = runner_with_an_sd_card_sample();
    report_missing(&mut runner, None, &[SD_CARD_SNARE]);
    runner.display.help_popup = None;

    assert_eq!(sd_card_mounted(&mut runner), vec![1]);
    report_missing(&mut runner, Some(1), &[SD_CARD_SNARE]);

    assert_eq!(sd_card_slot(&runner), NativeSampleAvailability::Unavailable);
    assert!(runner.display.help_popup.is_none());
}

#[test]
fn a_mount_keeps_the_popup_while_samples_elsewhere_are_still_missing() {
    let mut runner = runner_with_an_sd_card_sample();
    report_missing(
        &mut runner,
        None,
        &["userdata/User Kit/gone.wav", SD_CARD_SNARE],
    );

    assert_eq!(sd_card_mounted(&mut runner), vec![1]);
    report_loaded(&mut runner, 1, &[SD_CARD_SNARE]);

    assert_eq!(sd_card_slot(&runner), NativeSampleAvailability::Available);
    assert_eq!(
        runner.sample_availability[1][1],
        NativeSampleAvailability::Unavailable
    );
    assert!(runner.display.help_popup.is_some());
}

#[test]
fn a_mount_leaves_patches_without_card_samples_alone() {
    let mut runner = runner_with_two_samples();
    let _ = runner.messages_with_snapshot().unwrap();

    assert!(sd_card_mounted(&mut runner).is_empty());
}

#[test]
fn a_reload_of_one_instrument_does_not_mark_another_instrument_playing_the_same_file() {
    let mut runner = runner_with_an_sd_card_sample();
    runner.instruments[2].kind = InstrumentType::Sampler;
    runner.instruments[2].sample_paths[0] = Some(SD_CARD_SNARE.into());
    report_missing(&mut runner, None, &[SD_CARD_SNARE]);
    report_loaded(&mut runner, 2, &[SD_CARD_SNARE]);

    report_missing(&mut runner, Some(1), &[SD_CARD_SNARE]);

    assert_eq!(sd_card_slot(&runner), NativeSampleAvailability::Unavailable);
    assert_eq!(
        runner.sample_availability[2][0],
        NativeSampleAvailability::Available
    );
}

fn report_repaired(runner: &mut NativeRunner) {
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SdCardRepaired,
        })
        .unwrap();
}

fn press_main(runner: &mut NativeRunner) -> serde_json::Value {
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: serde_json::json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    crate::native_runner::tests::snapshot_from(&messages)
}

fn shown(runner: &mut NativeRunner) -> serde_json::Value {
    crate::native_runner::tests::snapshot_from(&runner.messages_with_snapshot().unwrap())
}

#[test]
fn a_repair_reported_at_boot_pops_up_after_the_splash_until_pressed_away() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.display.oled_splash_until = Some(Instant::now() + Duration::from_secs(5));
    assert_eq!(shown(&mut runner)["display"]["splash"], "startup");
    report_repaired(&mut runner);
    assert_eq!(press_main(&mut runner)["display"]["splash"], "startup");

    runner.display.oled_splash_until = Some(Instant::now() - Duration::from_millis(1));
    let snapshot = shown(&mut runner);
    assert_eq!(snapshot["display"]["splash"], "");
    assert_eq!(snapshot["display"]["title"], "SD card repaired");
    let text = runner.display.help_popup.as_ref().unwrap().lines.join(" ");
    assert!(text.contains("quick check tidied it up"), "{text}");
    assert_eq!(
        runner.display.toast.as_ref().unwrap().message,
        "Help: Sh+Fn+Main"
    );

    assert_ne!(
        press_main(&mut runner)["display"]["title"],
        "SD card repaired"
    );
    assert!(runner.display.help_popup.is_none());

    let mut next_boot = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let snapshot = shown(&mut next_boot);
    assert_eq!(snapshot["display"]["splash"], "");
    assert_ne!(snapshot["display"]["title"], "SD card repaired");
    assert!(next_boot.display.help_popup.is_none());
}

fn press_back(runner: &mut NativeRunner) -> serde_json::Value {
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: serde_json::json!({ "type": "button_a", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    crate::native_runner::tests::snapshot_from(&messages)
}

#[test]
fn a_repair_while_playing_pops_up_at_once_and_back_closes_it() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.display.aux_mapping_peek_visible = true;
    report_repaired(&mut runner);
    assert_eq!(shown(&mut runner)["display"]["title"], "SD card repaired");
    assert!(!runner.display.aux_mapping_peek_visible);

    assert_ne!(
        press_back(&mut runner)["display"]["title"],
        "SD card repaired"
    );
    assert!(runner.display.help_popup.is_none());
}

#[test]
fn a_repair_behind_the_missing_samples_popup_shows_once_the_card_samples_load() {
    let mut runner = runner_with_an_sd_card_sample();
    runner.skip_startup_splash();
    report_missing(&mut runner, None, &[SD_CARD_SNARE]);
    assert_eq!(sd_card_mounted(&mut runner), vec![1]);
    report_repaired(&mut runner);
    assert_eq!(shown(&mut runner)["display"]["title"], "Samples missing");

    report_loaded(&mut runner, 1, &[SD_CARD_SNARE]);
    assert_eq!(shown(&mut runner)["display"]["title"], "SD card repaired");
    assert_ne!(
        press_main(&mut runner)["display"]["title"],
        "SD card repaired"
    );
    assert!(runner.display.help_popup.is_none());
}

#[test]
fn the_missing_samples_popup_outranks_the_repair_popup_until_pressed_away() {
    let mut runner = runner_with_two_samples();
    runner.skip_startup_splash();
    report_repaired(&mut runner);
    report_missing(&mut runner, None, &["userdata/User Kit/gone.wav"]);
    assert_eq!(shown(&mut runner)["display"]["title"], "Samples missing");
    report_repaired(&mut runner);
    assert_eq!(shown(&mut runner)["display"]["title"], "Samples missing");

    assert_eq!(
        press_main(&mut runner)["display"]["title"],
        "SD card repaired"
    );
    assert_ne!(
        press_main(&mut runner)["display"]["title"],
        "SD card repaired"
    );
    assert!(runner.display.help_popup.is_none());
}
