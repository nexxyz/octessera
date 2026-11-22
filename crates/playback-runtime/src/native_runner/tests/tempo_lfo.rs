//! LFOs on BPM and Swing: they move only the clock, never the saved tempo, and stay out of the
//! way of an external clock.

use super::in_place_menu_actions::press_key;
use super::tempo_modulation::{
    press_play, published_bpm, saved_transport, set_menu_enum, set_menu_number,
};
use super::*;
use std::time::{Duration, Instant};

pub(super) fn lfo_on(runner: &mut NativeRunner, key: &str, period: &str, depth: i32) {
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
    runner
        .menu
        .set_bool_value_for_key("linkLfos.0.enabled", true);
    runner
        .apply_or_schedule_menu_key("linkLfos.0.enabled")
        .unwrap();
    assert!(runner.link_lfos[0].enabled);
}

pub(super) fn pulse(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 1,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap()
}

fn sent_commands(messages: &[RunnerMessage]) -> Vec<RuntimeAudioCommand> {
    messages
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::AudioCommands { commands } => commands.clone(),
            RunnerMessage::PlatformEffects { effects } => effects
                .iter()
                .filter_map(|effect| match effect {
                    RuntimePlatformEffect::AudioCommand { command } => Some(command.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        })
        .collect()
}

/// An LFO on BPM, five pulses in, so the clock runs off the set tempo.
fn runner_with_moving_bpm_lfo() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    lfo_on(&mut runner, "transport.bpm", "1/4", 50);
    press_play(&mut runner);
    for _ in 0..5 {
        pulse(&mut runner);
    }
    assert!((runner.clock_bpm() - runner.transport.bpm).abs() > 1.0);
    runner
}

#[test]
fn an_lfo_on_bpm_moves_the_clock_but_not_the_saved_tempo() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    lfo_on(&mut runner, "transport.bpm", "1/4", 50);
    let saved = saved_transport(&runner);
    press_play(&mut runner);
    runner.fast_autosave_marks = 0;

    let mut clock = Vec::new();
    let mut before = runner.clock_bpm();
    for _ in 0..24 {
        let messages = pulse(&mut runner);
        if runner.clock_bpm() != before {
            assert_eq!(published_bpm(&messages), Some(runner.clock_bpm()));
        }
        before = runner.clock_bpm();
        clock.push(before);
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
fn under_external_clock_an_lfo_on_bpm_does_nothing() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    lfo_on(&mut runner, "transport.bpm", "1/4", 100);
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
    assert!(
        (runner.clock_bpm() - 120.0).abs() > 1.0,
        "the LFO rides the internal clock"
    );

    set_menu_enum(&mut runner, "midiSyncMode", "external");
    assert_eq!(runner.transport.sync_source, SyncSource::External);
    assert_eq!(runner.clock_bpm(), 120.0);
    for _ in 0..12 {
        runner.send_midi_realtime_clock(1).unwrap();
        assert_eq!(runner.clock_bpm(), runner.transport.bpm);
    }
}

#[test]
fn a_bpm_lfo_leaves_note_mode_delays_and_held_play_fx_on_the_set_tempo() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.fx_buses[0].slots[0].fx_type = "delay".into();
    runner.fx_buses[0].slots[0].params = json!({ "timeMode": "note", "timeNote": "1/8", "timeMs": 250, "feedback": 0.35, "mixPct": 35 });
    runner.menu.rebuild(runner.menu_config());
    runner.play_fx_assignments.push(NativePlayFxAssignment {
        x: 0,
        y: 0,
        config: literal_play_fx(
            json!({ "fxType": "dub_throw", "targetKey": "master", "params": { "delayNote": 3 } }),
        ),
    });
    lfo_on(&mut runner, "transport.bpm", "1/4", 50);
    press_play(&mut runner);
    runner.active_play_mode = ActivePlayMode::Page(PlayMode::Fx);
    let held = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(
        sent_commands(&held)
            .iter()
            .any(|command| matches!(command, RuntimeAudioCommand::MomentaryFxStart { .. })),
        "the dub throw is held"
    );

    let mut clock = Vec::new();
    for _ in 0..24 {
        let commands = sent_commands(&pulse(&mut runner));
        clock.push(runner.clock_bpm());
        assert!(
            !commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::MomentaryFxUpdate { .. }
                    | RuntimeAudioCommand::SetFxBusParam {
                        param: realtime_engine::synth::FxParamId::TimeMs,
                        ..
                    }
            )),
            "{commands:?}"
        );
    }
    assert!(
        clock.iter().any(|bpm| (bpm - 120.0).abs() > 20.0),
        "{clock:?}"
    );
    assert_eq!(runner.fx_buses[0].slots[0].params["timeMs"], 250);
    assert_eq!(runner.play_fx_tempo_bpm(), 120.0);
}

#[test]
fn a_patch_clear_drops_the_bpm_lfo_from_the_clock() {
    let mut runner = runner_with_moving_bpm_lfo();
    runner.clear_patch_state().unwrap();
    assert_eq!(runner.clock_bpm(), runner.transport.bpm);
    assert_eq!(runner.transport.bpm, 120.0);
}

#[test]
fn a_patch_load_puts_the_clock_on_the_loaded_tempo() {
    let mut runner = runner_with_moving_bpm_lfo();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["transport"]["bpm"] = json!(90);
    runner.apply_config_payload(payload.clone()).unwrap();
    assert_eq!(runner.transport.bpm, 90.0);
    let offset = runner.clock_bpm() - 90.0;
    assert!(
        offset.abs() > 1.0 && offset.abs() <= 50.0,
        "the kept LFO rides 90: {offset}"
    );

    payload["runtimeConfig"]["linkLfos"][0]["enabled"] = json!(false);
    runner.apply_config_payload(payload).unwrap();
    assert!(!runner.link_lfos[0].enabled);
    assert_eq!(runner.clock_bpm(), 90.0);
}
