use super::device_config_apply::commit_setting;
use super::*;
use crate::native_runner::signal_view::SignalTapReader;
use crate::oled_frame::{
    OledPresentationMetrics, OledSignalArea, OledSignalView, OledSignalVisual,
};
use realtime_engine::synth::SignalTap;

const STEP: Duration = Duration::from_millis(66);

fn home_runner(start: Instant) -> (NativeRunner, &'static SignalTap) {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.display.transients.set_test_now(start);
    let tap: &'static SignalTap = Box::leak(Box::new(SignalTap::new()));
    runner.display.signal = SignalTapReader::new(tap);
    (runner, tap)
}

fn band(runner: &mut NativeRunner) -> Option<OledSignalView> {
    runner
        .capture_display_scene()
        .unwrap()
        .oled_presentation_input(OledPresentationMetrics::default(), None)
        .display
        .signal
}

fn sound(tap: &SignalTap) {
    let frames: Vec<f32> = (0..2048)
        .flat_map(|frame| {
            let sample = 0.5 * (frame as f32 * 0.07).sin();
            [sample, sample]
        })
        .collect();
    tap.publish_master(&frames, 2048, 48_000);
}

#[test]
fn the_stopped_home_page_shows_a_flat_band_and_schedules_nothing() {
    let start = Instant::now();
    let (mut runner, tap) = home_runner(start);
    assert_eq!(
        band(&mut runner),
        Some(OledSignalView {
            area: OledSignalArea::Band { slot: 4 },
            visual: OledSignalVisual::Flat
        })
    );
    assert_eq!(runner.display.signal.next_step_at(), None);
    sound(tap);
    assert_eq!(tap.written(), 0);
}

#[test]
fn a_playing_home_page_steps_its_scope_every_66_ms_until_the_transport_stops() {
    let start = Instant::now();
    let (mut runner, tap) = home_runner(start);
    runner.transport.transport = RuntimeTransportState::Playing;
    assert_eq!(band(&mut runner).unwrap().visual, OledSignalVisual::Flat);
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
    assert!(matches!(
        band(&mut runner).unwrap().visual,
        OledSignalVisual::Scope { .. }
    ));
    runner.display.transients.acknowledge_snapshot_pending();
    assert_eq!(
        runner.next_timed_display_snapshot_deadline_after(Some(start + STEP)),
        Some(start + STEP * 2)
    );

    runner.transport.transport = RuntimeTransportState::Stopped;
    assert_eq!(band(&mut runner).unwrap().visual, OledSignalVisual::Flat);
    assert_eq!(runner.display.signal.next_step_at(), None);
    let written = tap.written();
    sound(tap);
    assert_eq!(tap.written(), written);
}

#[test]
fn sub_pages_and_home_visual_none_show_no_band() {
    let (mut runner, _) = home_runner(Instant::now());
    runner.transport.transport = RuntimeTransportState::Playing;
    let _ = commit_setting(&mut runner, "homeVisual", -1);
    assert_eq!(runner.menu.value_for_key("homeVisual").unwrap(), "none");
    assert_eq!(runner.display.ui.home_visual, "none");
    runner.menu.state.stack.clear();
    assert_eq!(band(&mut runner), None);
    assert_eq!(runner.display.signal.next_step_at(), None);

    let _ = commit_setting(&mut runner, "homeVisual", 1);
    runner.menu.state.stack = vec![0];
    assert_eq!(band(&mut runner), None);
    runner.menu.state.stack.clear();
    assert!(band(&mut runner).is_some());
}

#[test]
fn home_visual_round_trips_through_the_saved_config() {
    let (mut runner, _) = home_runner(Instant::now());
    assert_eq!(runner.display.ui.home_visual, "scope");
    let _ = commit_setting(&mut runner, "homeVisual", 2);
    let payload = runner.config_payload();
    assert_eq!(payload["runtimeConfig"]["homeVisual"], "waterfall");

    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload).unwrap();
    assert_eq!(restored.display.ui.home_visual, "waterfall");
    assert_eq!(
        restored.menu.value_for_key("homeVisual").unwrap(),
        "waterfall"
    );
}

#[test]
fn spectrum_and_waterfall_draw_the_same_tap_once_it_sounds() {
    let start = Instant::now();
    let (mut runner, tap) = home_runner(start);
    runner.transport.transport = RuntimeTransportState::Playing;
    let _ = commit_setting(&mut runner, "homeVisual", 1);
    assert_eq!(runner.display.ui.home_visual, "spectrum");
    runner.menu.state.stack.clear();
    assert_eq!(band(&mut runner).unwrap().visual, OledSignalVisual::Flat);
    sound(tap);
    runner.display.transients.set_test_now(start + STEP);
    assert!(matches!(
        band(&mut runner).unwrap().visual,
        OledSignalVisual::Spectrum { .. }
    ));

    let _ = commit_setting(&mut runner, "homeVisual", 1);
    assert_eq!(runner.display.ui.home_visual, "waterfall");
    runner.menu.state.stack.clear();
    assert_eq!(band(&mut runner).unwrap().visual, OledSignalVisual::Flat);
    sound(tap);
    runner.display.transients.set_test_now(start + STEP * 2);
    assert!(matches!(
        band(&mut runner).unwrap().visual,
        OledSignalVisual::Waterfall { .. }
    ));
}
