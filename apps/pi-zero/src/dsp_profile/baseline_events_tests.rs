use super::{fx_events, mixer, sample_events, synth_events, BASELINE_FX_KINDS};
use realtime_engine::synth::{SampleBankConfig, VoiceStealingMode};
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
