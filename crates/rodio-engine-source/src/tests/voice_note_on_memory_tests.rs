use super::*;
use realtime_engine::synth::{FmConfig, PluckConfig, VoiceStealingMode};

fn slot(kind: &str) -> InstrumentSlotConfig {
    InstrumentSlotConfig {
        fm: (kind == "fm").then(FmConfig::default),
        pluck: (kind == "pluck").then(PluckConfig::default),
        drum: None,
        kind: kind.into(),
        synth: default_synth_config(),
        mixer: None,
    }
}

#[test]
fn synth_pluck_and_fm_note_ons_up_to_a_crowded_pool_do_not_touch_the_heap() {
    let (tx, rx) = event_queue();
    let mut source = EngineSource::new(rx, 44_100);
    tx.send(EngineEvent::SetPreparedAudioConfig {
        generation: 0,
        config: prepare_audio_config(
            InstrumentsConfig {
                instruments: vec![slot("synth"), slot("pluck"), slot("fm")],
                mixer: None,
                pan_positions: DEFAULT_PAN_POSITIONS,
                master_volume: 100.0,
            },
            Some(vec![SampleBankConfig::default(); 3]),
            None,
            44_100,
        ),
    })
    .unwrap();
    tx.send(EngineEvent::SetVoiceStealingMode {
        generation: 0,
        mode: VoiceStealingMode::None,
    })
    .unwrap();
    for _ in 0..1024 {
        let _ = source.next();
    }

    let (allocations, deallocations) = allocations_and_deallocations(|| {
        for note in 0..48u8 {
            tx.send(EngineEvent::NoteOn {
                instrument_slot: note % 3,
                note: 36 + note,
                velocity: 100,
                duration_ms: 5_000,
            })
            .unwrap();
            for _ in 0..512 {
                let _ = source.next();
            }
        }
    });

    assert_eq!((allocations, deallocations), (0, 0));
}
