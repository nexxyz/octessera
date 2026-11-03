use super::*;
use crate::native_runner::signal_view::monitor_source;
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

fn monitoring(key: &str) -> (NativeRunner, &'static SignalTap) {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    let tap: &'static SignalTap = Box::leak(Box::new(SignalTap::new()));
    runner.display.signal = SignalTapReader::new(tap);
    assert!(runner.menu.focus_item_key(key), "{key}");
    press_main(&mut runner);
    (runner, tap)
}

#[test]
fn monitor_rows_name_their_instrument_bus_or_the_master() {
    assert_eq!(
        monitor_source("monitor.instrument.3"),
        Some(SignalSource::Instrument(3))
    );
    assert_eq!(monitor_source("monitor.bus.1"), Some(SignalSource::Bus(1)));
    assert_eq!(monitor_source("monitor.global"), Some(SignalSource::Master));
    assert_eq!(monitor_source("monitor.layer.1"), None);
    assert_eq!(monitor_source("system.info"), None);
    for (key, source) in [
        ("monitor.instrument.0", SignalSource::Instrument(0)),
        ("monitor.bus.2", SignalSource::Bus(2)),
        ("monitor.global", SignalSource::Master),
    ] {
        let (runner, _) = monitoring(key);
        assert_eq!(
            runner
                .display
                .signal_monitor
                .as_ref()
                .map(|monitor| monitor.source),
            Some(source)
        );
    }
}

#[test]
fn a_stopped_monitor_fills_the_body_with_a_flat_line_under_its_page() {
    let (mut runner, tap) = monitoring("monitor.instrument.0");
    assert!(!runner.is_canonical_menu_presentation());
    let shown = presented(&mut runner);
    assert_eq!(shown.display.title, "../I1: synth/Scope");
    assert!(shown.display.lines.is_empty());
    let view = shown.display.signal.unwrap();
    assert_eq!(view.area, OledSignalArea::Body);
    assert_eq!(view.visual, crate::oled_frame::OledSignalVisual::Flat);
    assert_eq!(tap.selected(), None);
    assert_eq!(runner.display.signal.next_step_at(), None);
}

#[test]
fn main_cycles_the_picture_turns_do_nothing_and_back_closes() {
    let (mut runner, _) = monitoring("monitor.bus.0");
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
    assert!(runner.display.signal_monitor.is_none());
    assert_eq!(runner.menu.current_key(), Some("monitor.bus.0"));
    assert!(presented(&mut runner).display.signal.is_none());
}

#[test]
fn a_playing_monitor_taps_its_own_source() {
    let start = Instant::now();
    let (mut runner, tap) = monitoring("monitor.instrument.2");
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
        view.visual,
        crate::oled_frame::OledSignalVisual::Scope { .. }
    ));

    input(&mut runner, json!({ "type": "button_a", "pressed": true }));
    let _ = presented(&mut runner);
    assert_eq!(tap.selected(), None);
}

#[test]
fn modifier_chords_pass_through_and_leaving_the_page_closes_the_monitor() {
    let (mut runner, _) = monitoring("monitor.instrument.0");
    input(&mut runner, json!({ "type": "button_fn", "pressed": true }));
    press_main(&mut runner);
    input(
        &mut runner,
        json!({ "type": "button_fn", "pressed": false }),
    );
    let monitor = runner.display.signal_monitor.as_ref().unwrap();
    assert_eq!(monitor.kind.label(), "Scope");
    assert!(runner.display.help_popup.is_some());
    assert!(presented(&mut runner).display.signal.is_none());
    runner.display.help_popup = None;
    runner.display.aux_mapping_peek_visible = false;

    runner.menu.state.stack.clear();
    assert!(runner.is_canonical_menu_presentation());
    let shown = presented(&mut runner).display.signal.unwrap();
    assert_eq!(shown.area, OledSignalArea::Band { slot: 4 });
    press_main(&mut runner);
    assert!(runner.display.signal_monitor.is_none());
}

#[test]
fn an_aux_type_change_closes_the_monitor_before_it_shows_a_midi_source() {
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
    assert!(runner.menu.focus_item_key("monitor.instrument.0"));
    press_main(&mut runner);
    runner.transport.transport = RuntimeTransportState::Playing;
    let _ = presented(&mut runner);
    assert_eq!(tap.selected(), Some(SignalSource::Instrument(0)));

    for kind in ["sampler", "midi"] {
        input(
            &mut runner,
            json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
        );
        assert_eq!(runner.instruments[0].kind, kind);
        assert!(runner.shown_signal_monitor().is_none(), "{kind}");
        assert!(presented(&mut runner).display.signal.is_none(), "{kind}");
        assert_eq!(tap.selected(), None, "{kind}");
    }
    press_main(&mut runner);
    assert!(runner.display.signal_monitor.is_none());
}
