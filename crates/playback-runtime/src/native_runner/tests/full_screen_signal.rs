use super::*;
use crate::native_runner::signal_view::signal_row_source;
use crate::native_runner::signal_view::SignalTapReader;
use crate::oled_frame::{OledPresentationInput, OledPresentationMetrics, OledSignalArea};
use realtime_engine::synth::{SignalSource, SignalTap};

fn input(runner: &mut NativeRunner, input: Value) {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap();
}

fn press_main(runner: &mut NativeRunner) {
    input(runner, json!({ "type": "encoder_press", "id": "main" }));
}

fn presented(runner: &mut NativeRunner) -> OledPresentationInput {
    runner
        .capture_display_scene()
        .unwrap()
        .oled_presentation_input(OledPresentationMetrics::default(), None)
}

fn open_signal_view(key: &str) -> (NativeRunner, &'static SignalTap) {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    let tap: &'static SignalTap = Box::leak(Box::new(SignalTap::new()));
    runner.display.signal = SignalTapReader::new(tap);
    assert!(runner.menu.focus_item_key(key), "{key}");
    press_main(&mut runner);
    (runner, tap)
}

#[test]
fn signal_rows_name_their_instrument_bus_or_the_master() {
    assert_eq!(
        signal_row_source("signal.instrument.3"),
        Some(SignalSource::Instrument(3))
    );
    assert_eq!(
        signal_row_source("signal.bus.1"),
        Some(SignalSource::Bus(1))
    );
    assert_eq!(
        signal_row_source("signal.global"),
        Some(SignalSource::Master)
    );
    assert_eq!(signal_row_source("signal.layer.1"), None);
    assert_eq!(signal_row_source("system.info"), None);
    for (key, source) in [
        ("signal.instrument.0", SignalSource::Instrument(0)),
        ("signal.bus.2", SignalSource::Bus(2)),
        ("signal.global", SignalSource::Master),
    ] {
        let (runner, _) = open_signal_view(key);
        assert_eq!(
            runner
                .display
                .full_screen_signal
                .as_ref()
                .map(|view| view.source),
            Some(source)
        );
    }
}

#[test]
fn a_stopped_signal_view_fills_the_body_with_a_flat_line_under_its_page() {
    let (mut runner, tap) = open_signal_view("signal.instrument.0");
    assert!(!runner.is_canonical_menu_presentation());
    let shown = presented(&mut runner);
    assert_eq!(shown.display.title, "../I1: Synth/Scope");
    assert!(shown.display.lines.is_empty());
    let view = shown.display.signal.unwrap();
    assert_eq!(view.area, OledSignalArea::Body);
    assert_eq!(*view.visual, crate::oled_frame::OledSignalVisual::Flat);
    assert_eq!(tap.selected(), None);
    assert_eq!(runner.display.signal.next_step_at(), None);
}

#[test]
fn main_cycles_the_picture_turns_do_nothing_and_back_closes() {
    let (mut runner, _) = open_signal_view("signal.bus.0");
    let row = runner.menu.snapshot().selected_row;
    for label in ["Spectrum", "Waterfall", "Scope"] {
        press_main(&mut runner);
        assert!(presented(&mut runner).display.title.ends_with(label));
    }
    input(
        &mut runner,
        json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    );
    assert_eq!(runner.menu.snapshot().selected_row, row);

    input(&mut runner, json!({ "type": "button_a", "pressed": true }));
    input(&mut runner, json!({ "type": "button_a", "pressed": false }));
    assert!(runner.display.full_screen_signal.is_none());
    assert_eq!(runner.menu.current_key(), Some("signal.bus.0"));
    assert!(presented(&mut runner).display.signal.is_none());
}

#[test]
fn a_playing_signal_view_taps_its_own_source() {
    let start = Instant::now();
    let (mut runner, tap) = open_signal_view("signal.instrument.2");
    runner.display.transients.set_test_now(start);
    runner.transport.transport = RuntimeTransportState::Playing;
    let _ = presented(&mut runner);
    assert_eq!(tap.selected(), Some(SignalSource::Instrument(2)));
    let block: Vec<f32> = (0..2048).map(|frame| (frame as f32 * 0.05).sin()).collect();
    tap.publish_mono(block.into_iter(), 48_000);
    runner
        .display
        .transients
        .set_test_now(start + Duration::from_millis(66));
    let view = presented(&mut runner).display.signal.unwrap();
    assert!(matches!(
        *view.visual,
        crate::oled_frame::OledSignalVisual::Scope { .. }
    ));

    input(&mut runner, json!({ "type": "button_a", "pressed": true }));
    let _ = presented(&mut runner);
    assert_eq!(tap.selected(), None);
}

#[test]
fn modifier_chords_pass_through_and_leaving_the_page_closes_the_signal_view() {
    let (mut runner, _) = open_signal_view("signal.instrument.0");
    input(&mut runner, json!({ "type": "button_fn", "pressed": true }));
    press_main(&mut runner);
    input(
        &mut runner,
        json!({ "type": "button_fn", "pressed": false }),
    );
    let view = runner.display.full_screen_signal.as_ref().unwrap();
    assert_eq!(view.kind.label(), "Scope");
    assert!(runner.display.help_popup.is_some());
    assert!(presented(&mut runner).display.signal.is_none());
    runner.display.help_popup = None;
    runner.display.aux_mapping_peek_visible = false;

    runner.menu.state.stack.clear();
    assert!(runner.is_canonical_menu_presentation());
    let shown = presented(&mut runner).display.signal.unwrap();
    assert_eq!(shown.area, OledSignalArea::Band { slot: 4 });
    press_main(&mut runner);
    assert!(runner.display.full_screen_signal.is_none());
}

#[test]
fn an_aux_type_change_closes_the_signal_view_before_it_shows_a_midi_source() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    assert!(runner.menu.focus_item_key("aux:0:turn.instruments.0.type"));
    press_main(&mut runner);
    assert_eq!(
        runner.aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        Some("instruments.0.type")
    );
    let tap: &'static SignalTap = Box::leak(Box::new(SignalTap::new()));
    runner.display.signal = SignalTapReader::new(tap);
    assert!(runner.menu.focus_item_key("signal.instrument.0"));
    press_main(&mut runner);
    runner.transport.transport = RuntimeTransportState::Playing;
    let _ = presented(&mut runner);
    assert_eq!(tap.selected(), Some(SignalSource::Instrument(0)));

    for kind in ["sampler", "midi"] {
        input(
            &mut runner,
            json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
        );
        assert_eq!(runner.instruments[0].kind.wire_name(), kind);
        assert!(runner.shown_full_screen_signal().is_none(), "{kind}");
        assert!(presented(&mut runner).display.signal.is_none(), "{kind}");
        assert_eq!(tap.selected(), None, "{kind}");
    }
    press_main(&mut runner);
    assert!(runner.display.full_screen_signal.is_none());
}
