//! The shipped patch's Looper layer keeps the groove it was composed with.

use super::*;

/// Pulses (24 PPQN) after Play at which the factory L3 Looper sounds its note-ons over one loop.
/// The engine change that made the Looper prime at Play moved these one 1/16 earlier until the
/// stored steps were shifted back; this pins the groove itself, not the storage.
const FACTORY_L3_NOTE_ONS: [u32; 6] = [48, 54, 66, 96, 150, 168];

#[test]
fn the_factory_looper_layer_plays_its_composed_groove() {
    let payload: Value = serde_json::from_str(include_str!(
        "../../../../../../config/generated/pi/default.json"
    ))
    .unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.skip_startup_splash();
    let mut note_ons = Vec::new();
    let mut record = |pulse: u32, messages: Vec<RunnerMessage>| {
        for message in messages {
            if let RunnerMessage::MusicalEvents { events } = message {
                note_ons.extend(events.iter().filter_map(|event| {
                    matches!(
                        event,
                        platform_core::MusicalEvent::NoteOn { channel: 2, .. }
                    )
                    .then_some(pulse)
                }));
            }
        }
    };
    record(0, super::super::midi_note_in::press_play(&mut runner));
    for pulse in 1..192 {
        let messages = runner
            .send(HostMessage::TransportPulseStep {
                pulses: 1,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(false),
            })
            .unwrap();
        record(pulse, messages);
    }
    note_ons.dedup();
    assert_eq!(note_ons, FACTORY_L3_NOTE_ONS);
}
