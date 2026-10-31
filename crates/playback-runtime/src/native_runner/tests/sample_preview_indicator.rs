use super::audio_restart_persistence::press;
use super::in_place_menu_rows::runner_with_instrument;
use super::*;
use crate::oled_frame::{
    presentation_input_from_snapshot, render_oled_frame, OledPresentationInput,
    OledPresentationMetrics,
};
use std::time::{Duration, Instant};

fn browser_on_pad(start: Instant) -> NativeRunner {
    let mut runner = runner_with_instrument("sampler");
    runner.skip_startup_splash();
    runner.display.transients.set_test_now(start);
    assert!(runner.menu.focus_item_key("sample.choose:0:0"));
    press(&mut runner);
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SampleListResult {
                instrument_slot: 0,
                sample_slot: 0,
                dir: String::new(),
                entries: ["hat.wav", "pad.wav"]
                    .map(|name| SampleEntry {
                        name: name.into(),
                        path: name.into(),
                        is_dir: false,
                    })
                    .into(),
            },
        })
        .unwrap();
    assert!(runner.menu.focus_item_key("sample.pick.0.0.pad.wav"));
    runner
}

fn preview_started(runner: &mut NativeRunner, path: &str, duration_ms: u64) {
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SamplePreviewStarted {
                path: path.into(),
                duration_ms,
            },
        })
        .unwrap();
}

/// The typed scene and its JSON snapshot carry the same input; returns the typed one.
fn presentation(runner: &NativeRunner) -> OledPresentationInput {
    let metrics = OledPresentationMetrics::from_status(Some(0.5), true);
    let scene = runner.capture_presentation_scene(false).unwrap();
    let typed = scene.oled_presentation_input(metrics.clone(), None);
    let json = runner
        .capture_presentation_scene(false)
        .unwrap()
        .into_snapshot();
    let legacy = presentation_input_from_snapshot(&json, metrics)
        .unwrap()
        .unwrap();
    assert_eq!(typed, legacy);
    assert_eq!(render_oled_frame(&typed), render_oled_frame(&legacy));
    typed
}

fn selected_row_previews(runner: &NativeRunner) -> bool {
    let input = presentation(runner);
    let row = input.selected_row.unwrap();
    input.display.rows[row].as_ref().unwrap().preview
}

#[test]
fn the_preview_mark_shows_while_the_selected_file_sounds_not_while_play_is_pressed() {
    let start = Instant::now();
    let mut runner = browser_on_pad(start);

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects } if effects.iter().any(|effect| matches!(
            effect,
            RuntimePlatformEffect::AudioCommand {
                command: RuntimeAudioCommand::SamplePreview { path, .. }
            } if path == "pad.wav"
        ))
    )));
    assert!(!selected_row_previews(&runner));

    preview_started(&mut runner, "pad.wav", 800);
    assert!(selected_row_previews(&runner));
    let json = runner
        .capture_presentation_scene(false)
        .unwrap()
        .into_snapshot();
    let row = json["selectedRow"].as_u64().unwrap() as usize;
    assert_eq!(json["display"]["rows"][row]["preview"], true);

    assert!(runner.menu.focus_item_key("sample.pick.0.0.hat.wav"));
    assert!(!selected_row_previews(&runner));
    assert!(runner.menu.focus_item_key("sample.pick.0.0.pad.wav"));
    assert!(selected_row_previews(&runner));

    let end = start + Duration::from_millis(800);
    runner
        .display
        .transients
        .set_test_now(end - Duration::from_millis(1));
    assert!(selected_row_previews(&runner));
    runner.display.transients.set_test_now(end);
    assert!(!selected_row_previews(&runner));
}

#[test]
fn the_preview_end_is_a_scene_deadline_not_a_stopped_snapshot_deadline() {
    let start = Instant::now();
    let mut runner = browser_on_pad(start);
    preview_started(&mut runner, "pad.wav", 300);
    let end = start + Duration::from_millis(300);

    assert_eq!(
        runner.display.transients.next_deadline(Some(start)),
        Some(end)
    );
    assert!(runner
        .next_display_state_deadline_after(Some(start))
        .is_none_or(|deadline| deadline > end));

    runner.display.transients.set_test_now(end);
    assert!(runner.pending_display_scene_generation().is_some());
    assert!(!selected_row_previews(&runner));
}

#[test]
fn leaving_playing_silences_the_preview_and_its_mark() {
    let start = Instant::now();
    let mut runner = browser_on_pad(start);
    runner.transport.transport = RuntimeTransportState::Playing;
    preview_started(&mut runner, "pad.wav", 5_000);
    assert!(selected_row_previews(&runner));

    runner.transport.transport = RuntimeTransportState::Paused;
    runner.messages_with_snapshot().unwrap();
    assert!(!selected_row_previews(&runner));

    preview_started(&mut runner, "pad.wav", 5_000);
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.messages_with_snapshot().unwrap();
    assert!(selected_row_previews(&runner));
}
