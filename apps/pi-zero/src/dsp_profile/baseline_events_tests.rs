use super::{
    default_capacity_events, default_capacity_instruments, fx_events, mixed_ramp_16_48_events,
    mixer, sample_events, synth_events, BASELINE_FX_KINDS,
};
use realtime_engine::synth::{FxBusSlotConfig, SampleBankConfig, VoiceStealingMode};
use rodio_engine_source::EngineEvent;

fn note_on_slots(events: &[EngineEvent]) -> Vec<u8> {
    events
        .iter()
        .filter_map(|event| match event {
            EngineEvent::NoteOn {
                instrument_slot, ..
            } => Some(*instrument_slot),
            _ => None,
        })
        .collect()
}

#[test]
fn one_slot_synth_and_sample_fixtures_target_slot_zero() {
    let synth = synth_events(8, VoiceStealingMode::AutoBalanced, 44_100, 1);
    let sample_banks = vec![SampleBankConfig::default(); 8];
    let sample = sample_events(8, 44_100, &sample_banks, 1);

    assert_eq!(note_on_slots(&synth), vec![0; 8]);
    assert_eq!(note_on_slots(&sample), vec![0; 8]);
}

#[test]
fn mixed_boundary_fixture_has_two_synth_and_six_sampler_slots() {
    let sample_banks = vec![SampleBankConfig::default(); 8];
    let events = mixed_ramp_16_48_events(44_100, &sample_banks);
    let slots = note_on_slots(&events);

    assert_eq!(events.len(), 65);
    for slot in 0..2 {
        assert_eq!(
            slots
                .iter()
                .filter(|event_slot| **event_slot == slot)
                .count(),
            8
        );
    }
    for slot in 2..8 {
        assert_eq!(
            slots
                .iter()
                .filter(|event_slot| **event_slot == slot)
                .count(),
            8
        );
    }
    let synth_notes: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            EngineEvent::NoteOn {
                instrument_slot: 0..=1,
                note,
                ..
            } => Some(*note),
            _ => None,
        })
        .collect();
    assert_eq!(
        synth_notes,
        [60, 61, 62, 63, 64, 65, 66, 67, 60, 61, 62, 63, 64, 65, 66, 67]
    );
    assert!(events
        .iter()
        .skip(1)
        .filter_map(|event| match event {
            EngineEvent::NoteOn {
                instrument_slot: 2..=7,
                note,
                ..
            } => Some(*note),
            _ => None,
        })
        .all(|note| note == 36));
    match &events[0] {
        EngineEvent::SetPreparedAudioConfig(config) => {
            assert_eq!(config.sample_banks().unwrap().len(), 8);
        }
        _ => panic!("mixed boundary fixture did not begin with its audio config"),
    }
}

#[test]
fn baseline_fx_sequence_is_exact_and_six_slot_fixture_uses_its_prefix() {
    let six_mixer = mixer(6, 2);
    let six: Vec<_> = six_mixer.buses[..2]
        .iter()
        .flat_map(|bus| bus.slots.iter())
        .map(|slot| match slot {
            realtime_engine::synth::FxBusSlotConfig::Kind(kind) => kind.as_str(),
            realtime_engine::synth::FxBusSlotConfig::Config { .. } => "config",
        })
        .collect();
    let twelve = mixer(12, 2);

    assert_eq!(six, BASELINE_FX_KINDS[..6]);
    assert_eq!(
        twelve.buses.iter().flat_map(|bus| bus.slots.iter()).count(),
        12
    );
    assert_eq!(fx_events(6, 2, 0, 44_100, &[]).len(), 17);
}

#[test]
fn default_capacity_fixture_uses_shipped_slot_and_fx_topology() {
    let instruments = default_capacity_instruments(&[0, 2, 3, 4, 6, 7], &[1, 5]);
    assert_eq!(
        instruments
            .instruments
            .iter()
            .map(|slot| slot.kind.as_str())
            .collect::<Vec<_>>(),
        ["synth", "sampler", "synth", "synth", "synth", "sampler", "synth", "synth"]
    );
    assert_eq!(
        instruments
            .instruments
            .iter()
            .map(|slot| slot.mixer.as_ref().unwrap().route.as_str())
            .collect::<Vec<_>>(),
        [
            "fx_bus_1", "direct", "fx_bus_1", "fx_bus_2", "fx_bus_1", "direct", "fx_bus_1",
            "fx_bus_2",
        ]
    );
    let mixer = instruments.mixer.unwrap();
    let bus_kinds: Vec<_> = mixer
        .buses
        .iter()
        .flat_map(|bus| bus.slots.iter())
        .map(|slot| match slot {
            FxBusSlotConfig::Kind(kind) => kind.as_str(),
            FxBusSlotConfig::Config { .. } => "config",
        })
        .collect();
    assert_eq!(bus_kinds, ["delay", "duck", "duck", "saturator"]);
    assert_eq!(
        mixer
            .master
            .unwrap()
            .slots
            .iter()
            .map(|slot| match slot {
                FxBusSlotConfig::Kind(kind) => kind.as_str(),
                FxBusSlotConfig::Config { .. } => "config",
            })
            .collect::<Vec<_>>(),
        ["compressor"]
    );
    let events = default_capacity_events(&[0, 2, 3, 4, 6, 7], &[1, 5], 44_100, &[]);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, EngineEvent::PreparedMomentaryFxStart(_)))
            .count(),
        2
    );
}
