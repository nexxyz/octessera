use super::*;
use platform_core::MusicalEvent;

const PULSES: u32 = 8 * 96;

fn pi_default() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../config/generated/pi/default.json"
    ))
    .unwrap()
}

/// What `payload` plays for its first eight bars after Play: one line per sounding pulse,
/// `+slot.note.velocity.length` for a note-on (`h` for held), `-slot.note` for a note-off, `t` for
/// a tile hit, `m` for the MIDI lane and `@` for a swing offset.
pub(crate) fn timeline(payload: Value, midi: bool) -> String {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    if midi {
        runner.test_set_midi_enabled(true);
    }
    runner.skip_startup_splash();
    runner.send(HostMessage::TransportStop).unwrap();
    let play = device_input(&mut runner, json!({ "type": "button_s", "pressed": true }));
    let mut lines = Vec::new();
    push_line(&mut lines, 0, &play);
    for pulse in 1..=PULSES {
        let messages = runner
            .send(HostMessage::TransportPulseStep {
                pulses: 1,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(false),
            })
            .unwrap();
        push_line(&mut lines, pulse, &messages);
    }
    lines.join("\n") + "\n"
}

pub(crate) fn factory_timeline() -> String {
    timeline(pi_default(), false)
}

/// The factory patch with swing, its Sequencer bass moved to a hold slot of its own (a held
/// scan), its first Sequencer on a one-shot MIDI slot and its Life layer holding notes on another
/// unshared hold slot from Hold Trig while On and Off stay on the Synth. No two layers share a
/// hold slot, so the pin keeps out of the per-layer holder count's limit.
pub(crate) fn factory_variant_timeline() -> String {
    let mut payload = pi_default();
    let config = &mut payload["runtimeConfig"];
    config["transport"]["swingA"] = json!(5);
    config["transport"]["swingB"] = json!(3);
    for slot in [3, 4] {
        config["instruments"][slot]["noteBehavior"] = json!("hold");
    }
    let midi = &mut config["instruments"][7];
    midi["type"] = json!("midi");
    midi["midi"] = json!({ "channel": 8, "enabled": true, "velocity": 100 });
    config["layers"][1]["link"]["oneShotLengthMs"] = json!(200);
    for mapping in [
        "activate",
        "deactivate",
        "scanned",
        "scanned_empty",
        "stable",
    ] {
        config["layers"][1]["link"]["mapping"][mapping]["slot"] = json!(7);
        config["layers"][3]["link"]["mapping"][mapping]["slot"] = json!(4);
    }
    let stable = &mut config["layers"][0]["link"]["mapping"]["stable"];
    stable["slot"] = json!(3);
    stable["action"] = json!("note_on");
    timeline(payload, true)
}

pub(crate) fn push_line(lines: &mut Vec<String>, pulse: u32, messages: &[RunnerMessage]) {
    let mut words = Vec::new();
    for message in messages {
        match message {
            RunnerMessage::MusicalEvents { events } => words.extend(events.iter().map(word)),
            RunnerMessage::MidiEvents { events } => {
                words.extend(events.iter().map(|event| format!("m{}", word(event))));
            }
            RunnerMessage::TileHits { hits } => words.extend(hits.iter().map(|hit| {
                format!(
                    "t{}.{}.{}.{}.{}",
                    hit.instrument_slot, hit.voice, hit.tile, hit.pitch_cents, hit.velocity
                )
            })),
            RunnerMessage::SequencedOffset { nanos } => words.push(format!("@{nanos}")),
            _ => {}
        }
    }
    if !words.is_empty() {
        lines.push(format!("{pulse}: {}", words.join(" ")));
    }
}

fn word(event: &MusicalEvent) -> String {
    match event {
        MusicalEvent::NoteOn {
            channel,
            note,
            velocity,
            duration_ms,
        } => {
            let length = duration_ms.map_or_else(|| "h".to_string(), |ms| ms.to_string());
            format!("+{channel}.{note}.{velocity}.{length}")
        }
        MusicalEvent::NoteOff { channel, note } => format!("-{channel}.{note}"),
        MusicalEvent::Cc {
            channel,
            controller,
            value,
        } => format!("c{channel}.{controller}.{value}"),
    }
}

/// The factory patch is the first thing anyone hears. An engine change that moves one of its
/// notes, lengths or releases has to show up here and be judged by ear before the pin moves.
fn assert_pinned(actual: &str, expected: &str) {
    let first_difference = actual
        .lines()
        .zip(expected.lines())
        .position(|(actual, expected)| actual != expected);
    assert!(
        actual == expected,
        "first differing line {first_difference:?} of {} vs {}",
        actual.lines().count(),
        expected.lines().count()
    );
}

/// The factory patch is the first thing anyone hears. An engine change that moves one of its
/// notes, lengths or releases has to show up here and be judged by ear before the pin moves.
#[test]
pub(crate) fn the_factory_patch_plays_its_pinned_timeline() {
    assert_pinned(
        &factory_timeline(),
        include_str!("fixtures/factory_timeline.txt"),
    );
}

/// Pins the held-note paths the factory patch itself leaves unused: a held scan, Hold on another
/// instrument than Off, a MIDI one-shot slot and swing.
#[test]
pub(crate) fn the_factory_variant_plays_its_pinned_timeline() {
    assert_pinned(
        &factory_variant_timeline(),
        include_str!("fixtures/factory_variant_timeline.txt"),
    );
}
