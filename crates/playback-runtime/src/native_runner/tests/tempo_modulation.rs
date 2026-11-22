//! BPM and Swing as CC In, XY and LFO targets: held sources set the tempo like a menu edit, LFOs
//! ride on the clock without touching the saved tempo.

use super::in_place_menu_actions::press_key;
use super::*;
use crate::native_runner::modulation::source::ModulationSourceId;
use crate::{MidiControlChange, MidiControlChangeBatch};
use std::time::{Duration, Instant};

fn midi_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.test_set_midi_enabled(true);
    runner
}

fn send_cc(runner: &mut NativeRunner, cc: u8, value: u8) -> Vec<RunnerMessage> {
    let mut batch = MidiControlChangeBatch::default();
    batch.push(MidiControlChange::parse(&[0xB0, cc, value]).expect("a Control Change"));
    batch
        .into_iter()
        .flat_map(|change| runner.send(change.into()).unwrap())
        .collect()
}

fn set_menu_number(runner: &mut NativeRunner, key: &str, value: i32) {
    assert!(runner.menu.set_number_value_for_key(key, value), "{key}");
    runner.apply_or_schedule_menu_key(key).unwrap();
}

fn set_menu_enum(runner: &mut NativeRunner, key: &str, value: &str) {
    runner.menu.set_enum_value_for_key(key, value);
    runner.apply_or_schedule_menu_key(key).unwrap();
}

fn map_cc(runner: &mut NativeRunner, slot: usize, key: &str) {
    press_key(runner, &format!("midiCc.{slot}.target.{key}"));
    assert_eq!(
        runner.midi_cc_mappings[slot]
            .target
            .as_ref()
            .map(|binding| binding.key.as_str()),
        Some(key)
    );
    runner.midi_cc_mappings[slot].cc = u8::try_from(slot + 1).unwrap();
}

fn published_bpm(messages: &[RunnerMessage]) -> Option<f64> {
    messages.iter().rev().find_map(|message| match message {
        RunnerMessage::RuntimeConfigChanged { config } => Some(config.bpm),
        _ => None,
    })
}

fn saved_transport(runner: &NativeRunner) -> Value {
    runner.config_payload()["runtimeConfig"]["transport"].clone()
}

fn press_play(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    [true, false]
        .into_iter()
        .flat_map(|pressed| device_input(runner, json!({ "type": "button_s", "pressed": pressed })))
        .collect()
}

fn lfo_on(runner: &mut NativeRunner, key: &str, period: &str, depth: i32) {
    press_key(runner, &format!("linkLfos.0.target.{key}"));
    assert_eq!(
        runner.link_lfos[0].target.as_ref().map(|t| t.key.as_str()),
        Some(key),
        "{:?}",
        runner.display.toast
    );
    set_menu_enum(runner, "linkLfos.0.period", period);
    runner
        .menu
        .set_number_value_for_key("linkLfos.0.depthPct", depth);
    runner
        .apply_or_schedule_menu_key("linkLfos.0.depthPct")
        .unwrap();
    assert_eq!(i32::from(runner.link_lfos[0].depth_pct), depth);
    runner.link_lfos[0].enabled = true;
}

#[test]
fn a_cc_sets_bpm_and_swing_over_their_whole_range() {
    let mut runner = midi_runner();
    map_cc(&mut runner, 0, "transport.bpm");
    map_cc(&mut runner, 1, "transport.swingA");
    map_cc(&mut runner, 2, "transport.swingB");

    for (value, bpm) in [(0, 40.0), (64, 140.0), (127, 240.0)] {
        let messages = send_cc(&mut runner, 1, value);
        assert_eq!(runner.transport.bpm, bpm, "CC {value}");
        assert_eq!(runner.clock_bpm(), bpm);
        assert_eq!(published_bpm(&messages), Some(bpm), "the clock hears it");
    }
    for (value, swing) in [(0, 1), (64, 4), (127, 7)] {
        send_cc(&mut runner, 2, value);
        send_cc(&mut runner, 3, 127 - value);
        assert_eq!(runner.transport.swing_a, swing, "CC {value}");
        assert_eq!(runner.transport.swing_b, 8 - swing, "CC {value}");
    }
    assert_eq!(saved_transport(&runner)["bpm"], 240);
}

#[test]
fn a_cc_on_bpm_autosaves_like_any_held_target_only_when_the_tempo_moves() {
    let mut runner = midi_runner();
    map_cc(&mut runner, 0, "transport.bpm");
    runner.fast_autosave_marks = 0;

    send_cc(&mut runner, 1, 127);
    assert_eq!(runner.fast_autosave_marks, 1);
    send_cc(&mut runner, 1, 127);
    assert_eq!(
        runner.fast_autosave_marks, 1,
        "the same tempo is not a change"
    );
}

#[test]
fn the_last_edit_wins_between_the_bpm_row_and_a_cc() {
    let mut runner = midi_runner();
    map_cc(&mut runner, 0, "transport.bpm");
    send_cc(&mut runner, 1, 127);
    assert_eq!(runner.transport.bpm, 240.0);

    set_menu_number(&mut runner, "transport.bpm", 100);
    assert_eq!(runner.transport.bpm, 100.0);
    let source = ModulationSourceId::midi_cc(0).unwrap();
    assert!(!runner.modulation_process.has_source(source));
    assert_eq!(saved_transport(&runner)["bpm"], 100);

    send_cc(&mut runner, 1, 0);
    assert_eq!(runner.transport.bpm, 40.0, "a later CC takes it back");
}

#[test]
fn xy_sets_bpm_and_a_center_release_takes_the_middle_tempo() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    set_menu_enum(&mut runner, "play.xy.release", "reset-center");
    press_key(&mut runner, "xy:x.transport.bpm");
    assert_eq!(
        runner.xy.x_binding.as_ref().map(|b| b.key.as_str()),
        Some("transport.bpm")
    );
    runner.xy.smoothing_ms = 0;
    set_menu_number(&mut runner, "transport.bpm", 100);
    let now = Instant::now();

    runner.handle_play_xy_press_at(GRID_WIDTH - 1, 0, now);
    assert_eq!(runner.transport.bpm, 240.0);
    runner.handle_play_xy_press_at(0, 0, now);
    assert_eq!(runner.transport.bpm, 40.0);

    runner.handle_play_xy_release_at(now);
    assert_eq!(runner.transport.bpm, 140.0);
    set_menu_number(&mut runner, "transport.bpm", 101);
    assert_eq!(
        runner.transport.bpm, 101.0,
        "a menu edit takes it from the XY"
    );
}

#[test]
fn an_lfo_on_bpm_moves_the_clock_but_not_the_saved_tempo() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    lfo_on(&mut runner, "transport.bpm", "1/4", 50);
    let saved = saved_transport(&runner);
    press_play(&mut runner);
    runner.fast_autosave_marks = 0;

    let mut clock = Vec::new();
    for _ in 0..24 {
        let messages = runner
            .send(HostMessage::TransportPulseStep {
                pulses: 1,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(false),
            })
            .unwrap();
        assert_eq!(
            published_bpm(&messages).unwrap_or(runner.clock_bpm()),
            runner.clock_bpm()
        );
        clock.push(runner.clock_bpm());
    }
    let (low, high) = clock.iter().fold((f64::MAX, f64::MIN), |(low, high), bpm| {
        (low.min(*bpm), high.max(*bpm))
    });
    assert!(
        low < 100.0 && high > 140.0,
        "a 50% wave swings 50 BPM each way: {clock:?}"
    );
    assert_eq!(runner.transport.bpm, 120.0);
    assert_eq!(saved_transport(&runner), saved);
    assert_eq!(runner.fast_autosave_marks, 0);

    runner.send_transport_stop().unwrap();
    assert_eq!(runner.clock_bpm(), 120.0, "stop drops the LFO's tempo");
}

#[test]
fn an_lfo_on_bpm_rides_on_a_menu_edit() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    lfo_on(&mut runner, "transport.bpm", "1/4", 20);
    press_play(&mut runner);
    for _ in 0..6 {
        runner
            .send(HostMessage::TransportPulseStep {
                pulses: 1,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(false),
            })
            .unwrap();
    }
    let offset = runner.clock_bpm() - runner.transport.bpm;
    assert!(offset.abs() > 1.0, "{offset}");

    set_menu_number(&mut runner, "transport.bpm", 90);
    assert_eq!(runner.transport.bpm, 90.0);
    assert!((runner.clock_bpm() - (90.0 + offset)).abs() < 1e-9);
}

#[test]
fn an_lfo_on_swing_changes_the_swing_the_next_beat_takes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    lfo_on(&mut runner, "transport.swingA", "1/1", 100);
    press_play(&mut runner);
    let mut swings = Vec::new();
    for _ in 0..96 {
        runner
            .send(HostMessage::TransportPulseStep {
                pulses: 1,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(false),
            })
            .unwrap();
        swings.push(runner.clock_swing().0);
    }
    assert!(swings.contains(&1) && swings.contains(&7), "{swings:?}");
    assert_eq!(runner.transport.swing_a, 4);
}

/// An LFO on BPM, run through the runtime at the boards' 8 ms cadence and at 1 ms: the clock
/// keeps one pulse per wake at most, and its pulse spacing follows the wave.
#[test]
fn an_lfo_on_bpm_keeps_the_clock_steady() {
    for tick_ms in [1, 8] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        lfo_on(&mut runner, "transport.bpm", "1/1", 50);
        let mut runtime = crate::PlaybackRuntime::new(crate::RuntimeConfig {
            bpm: runner.transport.bpm,
            ..crate::RuntimeConfig::default()
        });
        let mut host = crate::tests::support::FakeHost::default();
        let start = Instant::now();
        runner.display.transients.set_test_now(start);
        let messages = press_play(&mut runner);
        runtime.ingest_runner_messages(messages, &mut host).unwrap();

        let tick = Duration::from_millis(tick_ms);
        let mut last_pulse = runner.transport.current_ppqn_pulse;
        let mut last_pulse_at = 0;
        let mut gaps = Vec::new();
        for step in 1..=4000 / tick_ms {
            runner
                .display
                .transients
                .set_test_now(start + tick * step as u32);
            runtime
                .advance_duration(tick, &mut runner, &mut host)
                .unwrap();
            assert_eq!(
                runtime.config().bpm,
                runner.clock_bpm(),
                "the clock hears the LFO"
            );
            let pulse = runner.transport.current_ppqn_pulse;
            assert!(
                pulse - last_pulse <= 1,
                "{tick_ms} ms: batched pulses at {step}"
            );
            if pulse > last_pulse {
                gaps.push((step - last_pulse_at) * tick_ms);
                last_pulse_at = step;
            }
            last_pulse = pulse;
        }
        let at_120 = 60_000 / (120 * 24);
        let shortest = gaps.iter().skip(1).min().copied().unwrap();
        let longest = gaps.iter().skip(1).max().copied().unwrap();
        if tick_ms == 1 {
            assert!(shortest < at_120 && longest > at_120 + 2, "{gaps:?}");
        }
        assert!(
            longest <= 60_000 / (60 * 24) + tick_ms,
            "{tick_ms} ms: a stall: {gaps:?}"
        );
    }
}

#[test]
fn under_external_clock_a_cc_on_bpm_sets_the_tempo_row_and_the_clock_ignores_it() {
    let mut runner = midi_runner();
    set_menu_enum(&mut runner, "midiSyncMode", "external");
    assert_eq!(runner.transport.sync_source, SyncSource::External);
    map_cc(&mut runner, 0, "transport.bpm");
    let mut runtime = crate::PlaybackRuntime::new(crate::RuntimeConfig {
        bpm: runner.transport.bpm,
        sync_source: SyncSource::External,
        ..crate::RuntimeConfig::default()
    });
    let mut host = crate::tests::support::FakeHost::default();
    let messages = press_play(&mut runner);
    runtime.ingest_runner_messages(messages, &mut host).unwrap();

    let messages = send_cc(&mut runner, 1, 127);
    runtime.ingest_runner_messages(messages, &mut host).unwrap();
    assert_eq!(
        runner.transport.bpm, 240.0,
        "as the BPM row does under external sync"
    );
    let before = runner.transport.current_ppqn_pulse;
    for _ in 0..100 {
        runtime
            .advance_duration(Duration::from_millis(10), &mut runner, &mut host)
            .unwrap();
    }
    assert_eq!(
        runner.transport.current_ppqn_pulse, before,
        "only the external clock moves it"
    );
}
