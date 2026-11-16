use super::device_config_apply::commit_setting;
use super::*;
use crate::native_menu::PlatformAction;
use crate::native_runner::signal_view::SignalTapReader;
use crate::oled_frame::{
    OledPresentationInput, OledPresentationMetrics, OledSignalArea, OledSignalVisual,
};
use realtime_engine::synth::{SignalSource, SignalTap};

const STEP: Duration = Duration::from_millis(66);

const PI_DEFAULT: &str = include_str!("../../../../../config/generated/pi/default.json");

/// The shipped board patch on the plain root menu, its signal tap a private one.
fn view_runner() -> (NativeRunner, &'static SignalTap) {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(serde_json::from_str(PI_DEFAULT).unwrap())
        .unwrap();
    runner.skip_startup_splash();
    let tap: &'static SignalTap = Box::leak(Box::new(SignalTap::new()));
    runner.display.signal = SignalTapReader::new(tap);
    (runner, tap)
}

fn back(runner: &mut NativeRunner) {
    device_input(runner, json!({ "type": "button_a", "pressed": true }));
    device_input(runner, json!({ "type": "button_a", "pressed": false }));
}

fn press_main(runner: &mut NativeRunner) {
    device_input(runner, json!({ "type": "encoder_press", "id": "main" }));
}

fn turn(runner: &mut NativeRunner, id: &str, delta: i32) {
    device_input(
        runner,
        json!({ "type": "encoder_turn", "id": id, "delta": delta }),
    );
}

fn held(runner: &mut NativeRunner, buttons: &[&str], action: impl FnOnce(&mut NativeRunner)) {
    for button in buttons {
        device_input(runner, json!({ "type": button, "pressed": true }));
    }
    action(runner);
    for button in buttons.iter().rev() {
        device_input(runner, json!({ "type": button, "pressed": false }));
    }
}

fn presented(runner: &mut NativeRunner) -> OledPresentationInput {
    runner
        .capture_display_scene()
        .unwrap()
        .oled_presentation_input(OledPresentationMetrics::default(), None)
}

fn toast(runner: &NativeRunner) -> Option<&str> {
    runner
        .display
        .toast
        .as_ref()
        .map(|toast| toast.message.as_str())
}

fn sound(tap: &SignalTap) {
    let frames: Vec<f32> = (0..4096)
        .flat_map(|frame| {
            let sample = 0.5 * (frame as f32 * 0.07).sin();
            [sample, sample]
        })
        .collect();
    tap.publish_master(&frames, 4096, 44_100);
}

#[test]
fn back_on_root_opens_the_view_and_back_returns_to_the_row_it_left() {
    let (mut runner, tap) = view_runner();
    turn(&mut runner, "main", 2);
    let row = runner.menu.state.cursor;
    back(&mut runner);
    assert!(runner.shows_performance_view());
    assert!(!runner.is_canonical_menu_presentation());
    let shown = presented(&mut runner);
    assert_eq!(shown.display.title, "L1: life");
    assert_eq!(shown.display.title_right, "120");
    assert!(shown.display.lines.is_empty());
    let signal = shown.display.signal.unwrap();
    assert_eq!(signal.area, OledSignalArea::Body);
    assert_eq!(*signal.visual, OledSignalVisual::Flat);
    assert_eq!(tap.selected(), None);
    let decoded = crate::oled_frame::presentation_input_from_snapshot(
        &runner.snapshot().unwrap(),
        OledPresentationMetrics::default(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(decoded.display.title_right, "120");

    back(&mut runner);
    assert!(!runner.shows_performance_view());
    assert!(runner.is_canonical_menu_presentation());
    assert_eq!(runner.menu.state.cursor, row);
    assert!(presented(&mut runner).display.title_right.is_empty());
}

#[test]
fn main_press_cycles_the_session_visual_through_none_and_the_home_band_follows() {
    let (mut runner, _) = view_runner();
    back(&mut runner);
    for (label, visual) in [
        ("Spectrum", Some("spectrum")),
        ("Waterfall", Some("waterfall")),
        ("No Visual", None),
    ] {
        press_main(&mut runner);
        assert_eq!(toast(&runner), Some(label));
        assert_eq!(
            runner
                .display
                .session_visual
                .map(|kind| kind.label().to_lowercase()),
            visual.map(String::from)
        );
    }
    assert!(presented(&mut runner).display.signal.is_none());
    turn(&mut runner, "main", 1);
    assert_eq!(runner.display.signal_tuning, Default::default());

    back(&mut runner);
    assert!(presented(&mut runner).display.signal.is_none());
    assert_eq!(runner.display.ui.home_visual, "scope");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["homeVisual"],
        "scope"
    );

    let _ = commit_setting(&mut runner, "homeVisual", 1);
    assert_eq!(runner.display.ui.home_visual, "spectrum");
    runner.menu.state.stack.clear();
    let band = presented(&mut runner).display.signal.unwrap();
    assert_eq!(band.area, OledSignalArea::Band { slot: 4 });
    back(&mut runner);
    press_main(&mut runner);
    assert_eq!(toast(&runner), Some("Waterfall"));
}

#[test]
fn a_stopped_view_schedules_no_frames_and_a_playing_one_steps_through_the_scene_pump() {
    let start = Instant::now();
    let (mut runner, tap) = view_runner();
    runner.display.transients.set_test_now(start);
    back(&mut runner);
    let _ = presented(&mut runner);
    runner.display.transients.acknowledge_snapshot_pending();
    assert_eq!(runner.display.signal.next_step_at(), None);
    assert!(runner
        .next_timed_display_snapshot_deadline_after(Some(start))
        .is_none_or(|deadline| deadline >= start + Duration::from_secs(1)));

    runner.transport.transport = RuntimeTransportState::Playing;
    let _ = presented(&mut runner);
    assert_eq!(tap.selected(), Some(SignalSource::Master));
    runner.display.transients.acknowledge_snapshot_pending();
    assert_eq!(
        runner.next_timed_display_snapshot_deadline_after(Some(start)),
        Some(start + STEP)
    );
    sound(tap);
    runner
        .display
        .transients
        .set_test_now(start + STEP - Duration::from_millis(1));
    assert_eq!(runner.pending_display_scene_generation(), None);
    runner.display.transients.set_test_now(start + STEP);
    assert!(runner.pending_display_scene_generation().is_some());
    let shown = presented(&mut runner).display.signal.unwrap();
    assert!(matches!(*shown.visual, OledSignalVisual::Scope { .. }));
}

#[test]
fn fn_right_column_in_the_view_activates_the_page_without_moving_the_menu() {
    let (mut runner, _) = view_runner();
    back(&mut runner);
    held(&mut runner, &["button_shift", "button_fn"], |runner| {
        grid_tap(runner, 7, 0);
    });
    assert_eq!(runner.active_play_mode, ActivePlayMode::Page(PlayMode::Fx));
    assert_eq!(toast(&runner), Some("FX"));
    assert!(runner.menu.state.stack.is_empty());
    assert!(runner.shows_performance_view());
    assert_eq!(presented(&mut runner).display.title, "L1: life  FX");

    held(&mut runner, &["button_fn"], |runner| grid_tap(runner, 0, 1));
    assert_eq!(runner.active_layer_index, 1);
    assert_eq!(runner.active_play_mode, ActivePlayMode::None);
    let title = presented(&mut runner).display.title;
    assert!(title.starts_with("L2: "), "{title}");

    back(&mut runner);
    held(&mut runner, &["button_fn"], |runner| grid_tap(runner, 7, 0));
    assert_eq!(runner.menu.current_key(), Some("play.page.mix"));
}

#[test]
fn aux_turns_in_the_view_follow_the_layer_or_play_fx_layout() {
    let (mut runner, _) = view_runner();
    assert!(runner.effective_aux_slot(1).turn.is_none());
    back(&mut runner);
    let count = "layers.0.build.behaviorConfig.randomCellsPerTick";
    let turn_key =
        |runner: &NativeRunner, index| runner.effective_aux_slot(index).turn.map(|turn| turn.key);
    assert_eq!(turn_key(&runner, 1).as_deref(), Some(count));
    let before = runner.active_layer().behavior_config["randomCellsPerTick"].clone();
    turn(&mut runner, "aux2", 1);
    assert_ne!(
        runner.active_layer().behavior_config["randomCellsPerTick"],
        before
    );
    assert!(runner.shows_performance_view());

    held(&mut runner, &["button_fn"], |runner| grid_tap(runner, 7, 0));
    assert_eq!(runner.active_play_mode, ActivePlayMode::Page(PlayMode::Mix));
    assert_eq!(turn_key(&runner, 1).as_deref(), Some(count));
    held(&mut runner, &["button_shift", "button_fn"], |runner| {
        grid_tap(runner, 7, 0);
    });
    assert!(turn_key(&runner, 0).is_some_and(|key| key.starts_with("play.fx.params.")));

    runner.aux_auto_map_enabled = false;
    assert_eq!(turn_key(&runner, 0), None);
}

#[test]
fn bound_clicks_run_in_the_view_except_signal_and_page_actions_leave_it() {
    let (mut runner, _) = view_runner();
    let bind = |runner: &mut NativeRunner, action| {
        runner.aux_bindings[0] = Some(NativeAuxBinding {
            turn_key: None,
            press_action: Some(NativeMenuAction::PlatformEffect(action)),
        });
    };
    back(&mut runner);
    bind(&mut runner, PlatformAction::Signal(SignalSource::Master));
    device_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "aux1" }),
    );
    assert_eq!(toast(&runner), Some("Clk-1: Not in view"));
    assert!(runner.display.full_screen_signal.is_none());
    assert!(runner.shows_performance_view());

    let slot = runner
        .instruments
        .iter()
        .position(|instrument| instrument.kind.wire_name() == "sampler")
        .expect("the default patch has a sampler");
    let sample_slot = runner.instruments[slot].selected_sample_slot;
    bind(
        &mut runner,
        PlatformAction::SampleOpen {
            instrument_slot: slot,
            sample_slot,
            dir: String::new(),
        },
    );
    device_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "aux1" }),
    );
    assert!(!runner.shows_performance_view());
    assert!(!runner.menu.state.stack.is_empty());
    while !runner.menu.state.stack.is_empty() {
        back(&mut runner);
    }
    assert!(!runner.shows_performance_view());

    back(&mut runner);
    bind(&mut runner, PlatformAction::MidiPanic);
    device_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "aux1" }),
    );
    assert_eq!(toast(&runner), Some("Clk-1: MIDI Panic"));
    assert!(runner.shows_performance_view());
}

#[test]
fn help_and_assignment_modes_sit_over_the_view_and_close_back_onto_it() {
    let (mut runner, _) = view_runner();
    back(&mut runner);
    held(&mut runner, &["button_shift", "button_fn"], press_main);
    let help = runner.display.help_popup.as_ref().unwrap();
    assert_eq!(help.title, "Help: Performance View");
    assert!(presented(&mut runner).display.signal.is_none());
    back(&mut runner);
    assert!(runner.display.help_popup.is_none());
    assert!(runner.shows_performance_view());

    runner.set_toast_for_test("Trigger Gate");
    let shown = presented(&mut runner);
    assert_eq!(shown.display.toast, "Trigger Gate");
    assert_eq!(shown.display.signal.unwrap().area, OledSignalArea::Body);

    runner
        .execute_platform_action(PlatformAction::TriggerProbabilityAssign(0))
        .unwrap();
    assert!(runner.grid_assign.is_some());
    back(&mut runner);
    assert!(runner.grid_assign.is_none());
    assert!(runner.shows_performance_view());
    back(&mut runner);
    assert!(!runner.shows_performance_view());
}

#[test]
fn view_on_back_off_keeps_back_on_root_a_plain_page_leave() {
    let (mut runner, _) = view_runner();
    let _ = commit_setting(&mut runner, "perfViewOnBack", -1);
    assert_eq!(
        runner.menu.value_for_key("perfViewOnBack").unwrap(),
        "false"
    );
    assert!(!runner.display.ui.perf_view_on_back);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["perfViewOnBack"],
        false
    );
    runner.menu.state.stack.clear();
    runner.active_play_mode = ActivePlayMode::Page(PlayMode::Mix);
    for _ in 0..3 {
        back(&mut runner);
        assert!(!runner.shows_performance_view());
    }
    assert_eq!(runner.active_play_mode, ActivePlayMode::None);
}

#[test]
fn view_at_boot_ends_the_startup_splash_in_the_stopped_view() {
    let (runner, _) = view_runner();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["perfViewAtBoot"] = true.into();
    let mut booted = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    booted.apply_config_payload(payload).unwrap();
    assert!(booted.display.ui.perf_view_at_boot);
    assert_eq!(booted.menu.value_for_key("perfViewAtBoot").unwrap(), "true");
    booted.display.oled_splash_until = Some(Instant::now() + Duration::from_secs(1));
    let messages = booted.messages_with_snapshot().unwrap();
    assert_eq!(snapshot_from(&messages)["display"]["splash"], "startup");
    assert!(!booted.shows_performance_view());

    booted.display.oled_splash_until = Some(Instant::now() - Duration::from_millis(1));
    let display = snapshot_from(&booted.messages_with_snapshot().unwrap())["display"].clone();
    assert_eq!(display["splash"], "");
    assert_eq!(display["toast"], "Help: Sh+Fn+Main");
    assert!(booted.shows_performance_view());
    let shown = presented(&mut booted);
    assert_eq!(shown.display.title_right, "120");
    assert_eq!(
        *shown.display.signal.unwrap().visual,
        OledSignalVisual::Flat
    );

    let (mut plain, _) = view_runner();
    plain.display.oled_splash_until = Some(Instant::now() - Duration::from_millis(1));
    plain.display.oled_mode = NativeOledMode::Splash;
    plain.display.oled_splash_text = OLED_STARTUP_SPLASH_KEY.into();
    let _ = plain.messages_with_snapshot().unwrap();
    assert!(!plain.shows_performance_view());
}
