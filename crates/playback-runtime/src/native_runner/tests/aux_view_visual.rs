use super::*;
use crate::native_runner::signal_view::SignalTapReader;
use crate::oled_frame::{
    OledAuxCard, OledAuxView, OledBarStyle, OledPresentationInput, OledPresentationMetrics,
};
use realtime_engine::synth::SignalTap;

const PI_DEFAULT: &str = include_str!("../../../../../config/generated/pi/default.json");
const HOLD: Duration = Duration::from_millis(1200);

/// The shipped board patch with the performance view open on its Aux visual.
fn aux_runner(start: Instant) -> (NativeRunner, &'static SignalTap) {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(serde_json::from_str(PI_DEFAULT).unwrap())
        .unwrap();
    runner.skip_startup_splash();
    runner.display.transients.set_test_now(start);
    let tap: &'static SignalTap = Box::leak(Box::new(SignalTap::new()));
    runner.display.signal = SignalTapReader::new(tap);
    press(&mut runner, "button_a");
    for _ in 0..3 {
        device_input(
            &mut runner,
            json!({ "type": "encoder_press", "id": "main" }),
        );
    }
    (runner.display.toast, runner.display.toast_expires_at) = (None, None);
    (runner, tap)
}

fn press(runner: &mut NativeRunner, button: &str) {
    device_input(runner, json!({ "type": button, "pressed": true }));
    device_input(runner, json!({ "type": button, "pressed": false }));
}

fn presented(runner: &mut NativeRunner) -> OledPresentationInput {
    runner
        .capture_display_scene()
        .unwrap()
        .oled_presentation_input(OledPresentationMetrics::default(), None)
}

fn cards(runner: &mut NativeRunner) -> [OledAuxCard; 3] {
    presented(runner)
        .display
        .aux_view
        .expect("the Aux visual")
        .cards
}

/// Each card's turn label, value, click and path, `-` for what is missing.
fn summary(cards: &[OledAuxCard; 3]) -> Vec<String> {
    cards
        .iter()
        .map(|card| {
            let turn = card.turn.as_ref();
            let value = turn.and_then(|turn| turn.gauge.as_ref());
            format!(
                "{} {} {} {}",
                turn.map_or("-", |turn| turn.label.as_str()),
                value.map_or("-", |gauge| gauge.value.as_str()),
                card.click.as_deref().unwrap_or("-"),
                turn.and_then(|turn| turn.path.as_deref()).unwrap_or("-"),
            )
        })
        .collect()
}

#[test]
fn the_aux_visual_mirrors_the_layer_auto_map_and_schedules_nothing() {
    let start = Instant::now();
    let (mut runner, tap) = aux_runner(start);
    assert!(runner.shows_aux_view());
    let shown = presented(&mut runner);
    assert!(shown.display.signal.is_none());
    assert_eq!(shown.display.title_right, "120");
    assert_eq!(shown.display.aux_view.as_ref().unwrap().bank, "Auto");
    let cards = shown.display.aux_view.unwrap().cards;
    assert_eq!(
        summary(&cards),
        ["Step 1/16 - -", "Count Off - -", "Interval 1 Spawn -"]
    );
    let step = cards[0].turn.as_ref().unwrap().gauge.as_ref().unwrap();
    assert!(
        (step.fraction - 3.0 / 11.0).abs() < 1.0e-6,
        "{}",
        step.fraction
    );
    assert!(cards.iter().all(|card| !card.turning));

    runner.transport.transport = RuntimeTransportState::Playing;
    let _ = presented(&mut runner);
    assert_eq!(tap.selected(), None);
    runner.display.transients.acknowledge_snapshot_pending();
    assert_eq!(runner.display.signal.next_step_at(), None);
    assert!(runner
        .next_timed_display_snapshot_deadline_after(Some(start))
        .is_none_or(|deadline| deadline >= start + Duration::from_secs(1)));
}

fn turn_main(runner: &mut NativeRunner, delta: i32) {
    device_input(
        runner,
        json!({ "type": "encoder_turn", "id": "main", "delta": delta }),
    );
}

fn toast(runner: &NativeRunner) -> Option<&str> {
    runner
        .display
        .toast
        .as_ref()
        .map(|toast| toast.message.as_str())
}

#[test]
fn main_turns_pick_a_strict_bank_that_holds_only_while_the_aux_visual_shows() {
    let (mut runner, _) = aux_runner(Instant::now());
    bind_manual_targets(&mut runner);
    turn_main(&mut runner, 1);
    assert_eq!(toast(&runner), Some("Aux: Manual"));
    let shown = presented(&mut runner).display;
    assert_eq!(shown.title_right, "120");
    assert_eq!(shown.aux_view.unwrap().bank, "Man");
    assert_eq!(
        summary(&cards(&mut runner)),
        [
            "Cutoff 151 - I1/Filter",
            "Mix 20% - B1/Reverb",
            "Pan Pos R3 - I6/Mixer"
        ]
    );
    let before = runner.active_layer().behavior_config["randomCellsPerTick"].clone();
    device_input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "aux2", "delta": -1 }),
    );
    assert_eq!(summary(&cards(&mut runner))[1], "Mix 15% - B1/Reverb");
    assert_eq!(
        runner.active_layer().behavior_config["randomCellsPerTick"],
        before
    );

    turn_main(&mut runner, -1);
    assert_eq!(toast(&runner), Some("Aux: Auto"));
    assert_eq!(
        summary(&cards(&mut runner)),
        ["Step 1/16 - -", "Count Off - -", "Interval 1 Spawn -"]
    );
    turn_main(&mut runner, 1);
    device_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert!(runner.aux_view_bank().is_none());
    assert_eq!(
        runner.effective_aux_slot(2).press.map(|press| press.label),
        Some("Spawn".into())
    );
    for _ in 0..4 {
        device_input(
            &mut runner,
            json!({ "type": "encoder_press", "id": "main" }),
        );
    }
    assert_eq!(presented(&mut runner).display.aux_view.unwrap().bank, "Man");
    runner.aux_auto_map_enabled = false;
    turn_main(&mut runner, -1);
    assert_eq!(
        summary(&cards(&mut runner)),
        ["- - - -", "- - - -", "- - - -"]
    );
}

#[test]
fn shift_held_shows_the_shift_layer_on_the_same_cards() {
    let (mut runner, _) = aux_runner(Instant::now());
    device_input(
        &mut runner,
        json!({ "type": "button_shift", "pressed": true }),
    );
    assert_eq!(
        summary(&cards(&mut runner)),
        ["Interval Off Glider -", "- - - -", "- - - -"]
    );
    device_input(
        &mut runner,
        json!({ "type": "button_shift", "pressed": false }),
    );
    assert_eq!(cards(&mut runner)[0].turn.as_ref().unwrap().label, "Step");
}

#[test]
fn an_aux_turn_updates_its_card_and_lights_it_instead_of_a_toast() {
    let start = Instant::now();
    let (mut runner, _) = aux_runner(start);
    device_input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "aux2", "delta": 1 }),
    );
    assert!(runner.display.toast.is_none());
    let cards = cards(&mut runner);
    assert_eq!(summary(&cards)[1], "Count 1 - -");
    assert!(cards[1].turning && !cards[0].turning && !cards[2].turning);
    let count = cards[1].turn.as_ref().unwrap().gauge.as_ref().unwrap();
    assert!(count.fraction > 0.0);

    runner.display.transients.acknowledge_snapshot_pending();
    assert_eq!(
        runner.next_display_state_deadline_after(Some(start)),
        Some(start + HOLD)
    );
    runner.display.transients.set_test_now(start + HOLD);
    assert!(!cards_of(&mut runner)[1].turning);

    device_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    device_input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "aux2", "delta": 1 }),
    );
    assert!(toast(&runner).is_some_and(|toast| toast.starts_with("Trn-2: Count: ")));
}

fn cards_of(runner: &mut NativeRunner) -> [OledAuxCard; 3] {
    cards(runner)
}

/// Binds Aux 1-3 to I1 Cutoff, B1 Reverb Mix and I6 Pan Pos with Fn + Aux press on their rows,
/// then returns to the Aux visual.
fn bind_manual_targets(runner: &mut NativeRunner) {
    press(runner, "button_a");
    for (key, aux) in [
        ("instruments.0.synth.filter.cutoffHz", "aux1"),
        ("mixer.buses.0.slot2.params.mixPct", "aux2"),
        ("instruments.5.mixer.panPos", "aux3"),
    ] {
        assert!(runner.menu.focus_item_key(key), "{key}");
        device_input(runner, json!({ "type": "button_fn", "pressed": true }));
        device_input(runner, json!({ "type": "encoder_press", "id": aux }));
        device_input(runner, json!({ "type": "button_fn", "pressed": false }));
    }
    runner.menu.state.stack.clear();
    runner.menu.state.cursor = 0;
    press(runner, "button_a");
    (runner.display.toast, runner.display.toast_expires_at) = (None, None);
}

#[test]
fn manual_bindings_show_their_owner_and_page_in_the_manual_bank() {
    let (mut runner, _) = aux_runner(Instant::now());
    bind_manual_targets(&mut runner);
    turn_main(&mut runner, 1);
    let cards = cards(&mut runner);
    assert_eq!(
        summary(&cards),
        [
            "Cutoff 151 - I1/Filter",
            "Mix 20% - B1/Reverb",
            "Pan Pos R3 - I6/Mixer"
        ]
    );
    let pan = cards[2].turn.as_ref().unwrap().gauge.as_ref().unwrap();
    assert_eq!(pan.style, OledBarStyle::Marker);
    let shown = presented(&mut runner);
    assert!(shown
        .aux_hints
        .normal
        .iter()
        .any(|hint| hint.turn.is_some()));
    assert!(shown.display.aux_view.is_some());
}

#[test]
fn compact_paths_name_the_owner_and_the_page_that_holds_the_row() {
    let (runner, _) = aux_runner(Instant::now());
    for (key, path) in [
        ("instruments.1.sample.slots.2.startPct", "I2/Sampler"),
        ("instruments.5.drum.voices.0.tuneSemis", "I6/Edit"),
        ("mixer.buses.0.panPos", "B1"),
        (
            "mixer.master.slots.0.params.thresholdDb",
            "Master/Compressor",
        ),
        (
            "layers.0.build.behaviorConfig.randomCellsPerTick",
            "L1/life",
        ),
        ("layers.1.algorithmStep", "L2/sequencer"),
        ("layers.0.link.x.filterCutoff.enabled", "L1/X Filter Cutoff"),
        ("linkLfos.1.depthPct", "LFO2"),
        ("midiCc.0.invert", "CC1"),
        ("play.xy.smoothingMs", "Play/XY"),
        ("masterVolume", "SYS/Audio"),
        ("transport.bpm", "Link"),
    ] {
        assert_eq!(runner.aux_compact_path(key).as_deref(), Some(path), "{key}");
    }
}

#[test]
fn the_home_band_keeps_the_stored_visual_while_the_view_shows_aux() {
    let (mut runner, _) = aux_runner(Instant::now());
    press(&mut runner, "button_a");
    assert!(!runner.shows_performance_view());
    let band = presented(&mut runner).display.signal.unwrap();
    assert!(matches!(
        band.area,
        crate::oled_frame::OledSignalArea::Band { .. }
    ));
    assert!(presented(&mut runner).display.aux_view.is_none());
    let _: Option<OledAuxView> = None;
}

#[test]
fn layer_and_page_select_keep_the_bank_and_only_the_auto_bank_follows_them() {
    let (mut runner, _) = aux_runner(Instant::now());
    bind_manual_targets(&mut runner);
    let raindrops = (0..8)
        .find(|layer| runner.layers[*layer].behavior_id == "raindrops")
        .unwrap();
    let select_layer = |runner: &mut NativeRunner, layer: usize| {
        device_input(runner, json!({ "type": "button_fn", "pressed": true }));
        grid_tap(runner, 0, layer);
        device_input(runner, json!({ "type": "button_fn", "pressed": false }));
    };
    turn_main(&mut runner, 1);
    let manual = summary(&cards(&mut runner));
    select_layer(&mut runner, raindrops);
    assert_eq!(runner.active_layer_index, raindrops);
    assert_eq!(summary(&cards(&mut runner)), manual);
    assert_eq!(runner.aux_view_bank(), Some(AuxPriority::Manual));

    turn_main(&mut runner, -1);
    let raindrops_cards = summary(&cards(&mut runner));
    assert!(
        raindrops_cards[2].ends_with("Drop -"),
        "{raindrops_cards:?}"
    );
    device_input(
        &mut runner,
        json!({ "type": "button_shift", "pressed": true }),
    );
    device_input(&mut runner, json!({ "type": "button_fn", "pressed": true }));
    grid_tap(&mut runner, 7, 0);
    device_input(
        &mut runner,
        json!({ "type": "button_fn", "pressed": false }),
    );
    device_input(
        &mut runner,
        json!({ "type": "button_shift", "pressed": false }),
    );
    assert_eq!(runner.active_play_mode, ActivePlayMode::Page(PlayMode::Fx));
    assert_eq!(runner.aux_view_bank(), Some(AuxPriority::Auto));
    let header = presented(&mut runner).display;
    assert_eq!(header.title, "L6: raindrops  FX");
    assert_eq!(header.title_right, "120");
    assert_eq!(header.aux_view.unwrap().bank, "Auto");
    let fx = cards(&mut runner);
    assert_eq!(fx[0].click.as_deref(), Some("Map"));
    assert_ne!(summary(&fx), raindrops_cards);
    select_layer(&mut runner, 0);
    assert_eq!(
        summary(&cards(&mut runner)),
        ["Step 1/16 - -", "Count Off - -", "Interval 1 Spawn -"]
    );
}
