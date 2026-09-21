use super::*;
use realtime_engine::synth::{
    default_synth_config, prepare_instrument_slot_config, InstrumentSlotConfig, SampleBuffer,
    DEFAULT_PAN_POSITIONS,
};

impl EngineSource {
    fn fill_retirement_storage_for_transport_test(&mut self) {
        for _ in 0..RETIREMENT_QUEUE_CAPACITY {
            self.retired_tx
                .try_send(RetiredAudioItem {
                    state: None,
                    event: None,
                    #[cfg(test)]
                    drop_probe: None,
                })
                .unwrap();
        }
        for _ in 0..RETIREMENT_BACKLOG_CAPACITY {
            self.retired_backlog
                .as_mut()
                .expect("retired backlog")
                .enqueue(RetiredAudioItem {
                    state: None,
                    event: None,
                    #[cfg(test)]
                    drop_probe: None,
                });
        }
        assert_eq!(self.retired_backlog_len(), RETIREMENT_BACKLOG_CAPACITY);
    }
}

fn prepared_slot() -> realtime_engine::synth::PreparedInstrumentSlot {
    prepare_instrument_slot_config(InstrumentSlotConfig {
        kind: "synth".into(),
        synth: default_synth_config(),
        mixer: None,
    })
}

#[test]
fn retirement_saturation_blocks_owner_work_but_not_musical_work() {
    let (tx, rx) = event_queue();
    let (mut source, _retired_rx) = EngineSource::with_test_retirement_receiver(rx, 44_100);
    source.fill_retirement_storage_for_transport_test();
    tx.send(EngineEvent::SetPreparedAudioConfig {
        generation: 1,
        config: realtime_engine::synth::prepare_audio_config(
            realtime_engine::synth::InstrumentsConfig {
                instruments: Vec::new(),
                mixer: None,
                pan_positions: DEFAULT_PAN_POSITIONS,
                master_volume: 100.0,
            },
            None,
            None,
            44_100,
        ),
    })
    .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 100,
    })
    .unwrap();
    assert_eq!(source.drain_control_events().control_events, 1);
    assert!(source.engine.profile_snapshot().active_synth_voices > 0);
    assert!(source.control_rx.try_recv().is_ok());
}

#[test]
fn retirement_disconnect_fails_closed_for_structural_work() {
    let (tx, rx) = event_queue();
    let (mut source, retired_rx) = EngineSource::with_test_retirement_receiver(rx, 44_100);
    source
        .retired_backlog
        .as_mut()
        .expect("retired backlog")
        .enqueue(RetiredAudioItem {
            state: None,
            event: None,
            #[cfg(test)]
            drop_probe: None,
        });
    drop(retired_rx);
    tx.send(EngineEvent::SetPreparedInstrumentSlot {
        instrument_slot: 0,
        generation: 1,
        config: prepared_slot(),
    })
    .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 100,
    })
    .unwrap();
    assert_eq!(source.drain_control_events().control_events, 1);
    assert!(source.control_rx.try_recv().is_ok());
}

#[test]
fn pending_render_retirement_is_deferred_when_storage_is_full() {
    let (_tx, rx) = event_queue();
    let (mut source, _actual_retired_rx) = EngineSource::with_test_retirement_receiver(rx, 44_100);
    let _ = source.engine.preview_sample(
        0,
        SampleBuffer {
            samples: vec![0.25].into(),
            channels: 1,
            sample_rate: 44_100,
        },
        100,
    );
    source.fill_retirement_storage_for_transport_test();
    let _ = source.next();
    assert_eq!(source.retired_backlog_len(), RETIREMENT_BACKLOG_CAPACITY);
}
