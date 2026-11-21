use super::*;
use platform_core::MusicalEvent;

const PULSES: u32 = 8 * 96;

/// What the shipped Pi default patch plays for its first eight bars after Play: one line per
/// sounding pulse, `+slot.note.velocity.length` for a note-on (`h` for held), `-slot.note` for a
/// note-off, `t` for a tile hit and `m` for the MIDI lane.
pub(crate) fn factory_timeline() -> String {
    let payload: Value = serde_json::from_str(include_str!(
        "../../../../../config/generated/pi/default.json"
    ))
    .unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
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

fn push_line(lines: &mut Vec<String>, pulse: u32, messages: &[RunnerMessage]) {
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
#[test]
pub(crate) fn the_factory_patch_plays_its_pinned_timeline() {
    let actual = factory_timeline();
    let expected = include_str!("fixtures/factory_timeline.txt");
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
