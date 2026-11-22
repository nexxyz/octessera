//! The LFO option sweep: every Period at Depth 0, the default and 100 must move each kind of
//! target, through the menu rows and the transport the hardware drives.

use super::in_place_menu_actions::press_key;
use super::*;
use std::time::{Duration, Instant};

const TARGETS: [&str; 5] = [
    "instruments.0.mixer.volume",
    "instruments.0.synth.filter.cutoffHz",
    "instruments.0.synth.amp.gainPct",
    "instruments.0.synth.filter.resonance",
    "instruments.1.sample.filter.cutoffHz",
];
const SWEEP_PULSES: u32 = 48;
const STEPS_PER_PULSE: u32 = 4;

fn set_menu_number(runner: &mut NativeRunner, key: &str, value: i32) {
    runner.menu.set_number_value_for_key(key, value);
    assert_eq!(runner.menu.number_for_key(key), Some(value), "{key}");
    runner.apply_or_schedule_menu_key(key).unwrap();
}

fn set_menu_enum(runner: &mut NativeRunner, key: &str, value: &str) {
    runner.menu.set_enum_value_for_key(key, value);
    assert_eq!(
        runner.menu.value_for_key(key).as_deref(),
        Some(value),
        "{key}"
    );
    runner.apply_or_schedule_menu_key(key).unwrap();
}

/// The value an audio command sends to `target`, if it is about it.
fn target_value(command: &RuntimeAudioCommand, target: &str) -> Option<f32> {
    let (index, field) = target
        .strip_prefix("instruments.")?
        .split_once('.')
        .map(|(index, field)| (index.parse::<usize>().ok(), field))?;
    match command {
        RuntimeAudioCommand::SetInstrumentMixer {
            instrument_slot,
            volume_pct: Some(volume),
            ..
        } if Some(*instrument_slot) == index && field == "mixer.volume" => Some(*volume),
        RuntimeAudioCommand::SetSynthParam {
            instrument_slot,
            path,
            value,
            ..
        }
        | RuntimeAudioCommand::SetSampleBankParam {
            instrument_slot,
            path,
            value,
            ..
        } if Some(*instrument_slot) == index && path == field => Some(*value),
        _ => None,
    }
}

/// What `target` plays at each quarter pulse over `SWEEP_PULSES` pulses with LFO1 on it.
fn sweep(target: &str, period: &str, depth: i32) -> Vec<f32> {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    if target.contains(".sample.") {
        runner.instruments[1].kind = InstrumentType::Sampler;
        runner.menu.rebuild(runner.menu_config());
    }
    let Some(crate::native_menu::NativeMenuValue::Number { min, max, .. }) =
        runner.menu.item_for_key(target).map(|item| item.value)
    else {
        panic!("{target} is not a number row");
    };
    set_menu_number(&mut runner, target, (min + max) / 2);
    press_key(&mut runner, &format!("linkLfos.0.target.{target}"));
    assert_eq!(
        runner.link_lfos[0].target.as_ref().map(|t| t.key.as_str()),
        Some(target)
    );
    set_menu_enum(&mut runner, "linkLfos.0.period", period);
    set_menu_number(&mut runner, "linkLfos.0.depthPct", depth);
    runner.link_lfos[0].enabled = true;
    let start = Instant::now();
    runner.display.transients.set_test_now(start);
    let _ = device_input(&mut runner, json!({ "type": "button_s", "pressed": true }));
    let _ = device_input(&mut runner, json!({ "type": "button_s", "pressed": false }));
    assert_eq!(runner.transport.transport, RuntimeTransportState::Playing);
    assert!(runner.link_lfos[0].enabled && runner.link_lfos[0].depth_pct as i32 == depth);
    let pulse_time = Duration::from_secs_f64(60.0 / (runner.transport.bpm * 24.0));
    let mut value = None;
    let mut values = Vec::new();
    for pulse in 0..SWEEP_PULSES {
        for step in 0..STEPS_PER_PULSE {
            let now = start + pulse_time * pulse + pulse_time * step / STEPS_PER_PULSE;
            runner.display.transients.set_test_now(now);
            let messages = runner
                .send(HostMessage::TransportPulseStep {
                    pulses: u32::from(step == 0),
                    source: SyncSource::Internal,
                    at_ppqn_pulse: None,
                    request_snapshot: Some(false),
                })
                .unwrap();
            for message in messages {
                if let RunnerMessage::AudioCommands { commands } = message {
                    for command in commands {
                        value = target_value(&command, target).or(value);
                    }
                }
            }
            values.push(value.unwrap_or(f32::NAN));
        }
    }
    values
}

fn levels(values: &[f32]) -> usize {
    let mut levels = values
        .iter()
        .filter(|value| value.is_finite())
        .map(|value| (value * 100.0).round() as i64)
        .collect::<Vec<_>>();
    levels.sort_unstable();
    levels.dedup();
    levels.len()
}

#[test]
fn every_period_and_depth_moves_every_kind_of_lfo_target() {
    for target in TARGETS {
        let mut by_period = Vec::new();
        for period in crate::timing_units::LFO_PERIOD_OPTIONS {
            let still = sweep(target, period, 0);
            assert!(
                levels(&still) <= 1,
                "{target} {period}: depth 0 moved it: {still:?}"
            );
            let mut previous = None;
            for depth in [50, 100] {
                let moving = sweep(target, period, depth);
                let (low, high) = moving.iter().fold((f32::MAX, f32::MIN), |(low, high), v| {
                    (low.min(*v), high.max(*v))
                });
                assert!(low < high, "{target} {period} depth {depth}: flat");
                assert!(
                    levels(&moving) >= 3,
                    "{target} {period} depth {depth}: only {} levels: {moving:?}",
                    levels(&moving)
                );
                assert_ne!(
                    previous.as_ref(),
                    Some(&moving),
                    "{target} {period}: depth {depth} sounds like the depth before"
                );
                previous = Some(moving);
            }
            by_period.push((period, previous.unwrap()));
        }
        for pair in by_period.windows(2) {
            assert_ne!(
                pair[0].1, pair[1].1,
                "{target}: {} and {} play the same wave",
                pair[0].0, pair[1].0
            );
        }
    }
}

#[test]
fn an_8_ms_runtime_tick_moves_a_1_32t_lfo_between_its_pulses() {
    let target = "instruments.0.mixer.volume";
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    set_menu_number(&mut runner, target, 50);
    press_key(&mut runner, &format!("linkLfos.0.target.{target}"));
    set_menu_enum(&mut runner, "linkLfos.0.period", "1/32T");
    set_menu_number(&mut runner, "linkLfos.0.depthPct", 100);
    runner.link_lfos[0].enabled = true;
    let mut runtime = crate::PlaybackRuntime::new(crate::RuntimeConfig {
        bpm: runner.transport.bpm,
        ..crate::RuntimeConfig::default()
    });
    let mut host = crate::tests::support::FakeHost::default();
    let start = Instant::now();
    runner.display.transients.set_test_now(start);
    for pressed in [true, false] {
        let messages = device_input(
            &mut runner,
            json!({ "type": "button_s", "pressed": pressed }),
        );
        runtime.ingest_runner_messages(messages, &mut host).unwrap();
    }
    assert_eq!(runner.transport.transport, RuntimeTransportState::Playing);
    host.audio_commands.clear();

    let tick = Duration::from_millis(8);
    for step in 1..=60 {
        runner.display.transients.set_test_now(start + tick * step);
        runtime
            .advance_duration(tick, &mut runner, &mut host)
            .unwrap();
    }
    let volumes = host
        .audio_commands
        .iter()
        .filter_map(|command| target_value(command, target))
        .collect::<Vec<_>>();
    assert!(
        levels(&volumes) >= 3,
        "a 1/32T wave, which pulses alone hold at one level, at 8 ms ticks: {volumes:?}"
    );
}
