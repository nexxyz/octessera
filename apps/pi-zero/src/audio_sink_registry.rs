use crate::audio::AudioSink;
use crate::audio_replay::{is_replay_event, replay_to_sink, ReplayCache};
use rodio_engine_source::{EngineEvent, EngineEventSender};
use std::sync::{Arc, Mutex};

pub(crate) type AudioAttachGate = Arc<Mutex<()>>;

pub(crate) fn new_attach_gate() -> AudioAttachGate {
    Arc::new(Mutex::new(()))
}

pub(crate) struct SinkSender {
    pub(crate) sink: AudioSink,
    tx: EngineEventSender,
}

#[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn test_sink_sender(tx: EngineEventSender) -> SinkSender {
    test_sink_sender_for(AudioSink::Jack, tx)
}

#[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn test_sink_sender_for(sink: AudioSink, tx: EngineEventSender) -> SinkSender {
    SinkSender { sink, tx }
}

pub(crate) fn broadcast_event(
    txs: &Arc<Mutex<Vec<SinkSender>>>,
    event: EngineEvent,
) -> Result<(), String> {
    let mut failed = Vec::new();
    let mut first_error = None;
    let mut guard = txs
        .lock()
        .map_err(|_| "audio sink registry lock failed".to_string())?;
    for sink in guard.iter() {
        if let Err(error) = sink.tx.send(event.clone()) {
            first_error.get_or_insert_with(|| error.to_string());
            failed.push(sink.sink);
        }
    }
    guard.retain(|sink| !failed.contains(&sink.sink));
    if failed.is_empty() || !failed.iter().any(failed_sink_is_required) {
        Ok(())
    } else {
        Err(format!(
            "audio event queue unavailable for {:?}: {}",
            failed,
            first_error.unwrap_or_else(|| "unknown queue failure".into())
        ))
    }
}

pub(crate) fn broadcast_event_atomic(
    gate: &AudioAttachGate,
    txs: &Arc<Mutex<Vec<SinkSender>>>,
    replay_events: &Arc<Mutex<ReplayCache>>,
    event: EngineEvent,
) -> Result<(), String> {
    let _gate = gate
        .lock()
        .map_err(|_| "audio attach gate lock failed".to_string())?;
    if is_replay_event(&event) {
        replay_events
            .lock()
            .map_err(|_| "audio replay cache lock failed".to_string())?
            .remember(&event);
    }
    broadcast_event(txs, event)
}

fn failed_sink_is_required(sink: &AudioSink) -> bool {
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    {
        *sink == AudioSink::Jack
    }
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    {
        let _ = sink;
        true
    }
}

pub(crate) fn register_sink(
    txs: &Arc<Mutex<Vec<SinkSender>>>,
    sink: AudioSink,
    tx: EngineEventSender,
) {
    if let Ok(mut txs) = txs.lock() {
        txs.retain(|entry| entry.sink != sink);
        txs.push(SinkSender { sink, tx });
    }
}

pub(crate) fn remove_sink(txs: &Arc<Mutex<Vec<SinkSender>>>, sink: AudioSink) {
    if let Ok(mut txs) = txs.lock() {
        txs.retain(|entry| entry.sink != sink);
    }
}

pub(crate) fn remove_sink_atomic(
    gate: &AudioAttachGate,
    txs: &Arc<Mutex<Vec<SinkSender>>>,
    sink: AudioSink,
) -> Result<(), String> {
    let _gate = gate
        .lock()
        .map_err(|_| "audio attach gate lock failed".to_string())?;
    remove_sink(txs, sink);
    Ok(())
}

pub(crate) fn has_sink(txs: &Arc<Mutex<Vec<SinkSender>>>, sink: AudioSink) -> bool {
    txs.lock()
        .map(|txs| txs.iter().any(|entry| entry.sink == sink))
        .unwrap_or(false)
}

pub(crate) fn attach_sink_atomic(
    gate: &AudioAttachGate,
    txs: &Arc<Mutex<Vec<SinkSender>>>,
    replay_events: &Arc<Mutex<ReplayCache>>,
    sink: AudioSink,
    tx: EngineEventSender,
) -> Result<(), String> {
    let _gate = gate
        .lock()
        .map_err(|_| "audio attach gate lock failed".to_string())?;
    replay_to_sink(&tx, replay_events)?;
    register_sink(txs, sink, tx);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio_engine_source::event_queue;

    #[test]
    fn atomic_attach_delivers_a_concurrent_event_exactly_once() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let gate = new_attach_gate();
        let txs = Arc::new(Mutex::new(Vec::new()));
        let replay = Arc::new(Mutex::new(ReplayCache::default()));
        let (sink_tx, mut sink_rx) = event_queue();
        let start = Arc::new(Barrier::new(3));
        let event = EngineEvent::SetMasterVolume { volume_pct: 72.0 };
        let event_gate = gate.clone();
        let event_txs = txs.clone();
        let event_replay = replay.clone();
        let event_start = start.clone();
        let event_thread = thread::spawn(move || {
            event_start.wait();
            broadcast_event_atomic(&event_gate, &event_txs, &event_replay, event).unwrap();
        });
        let attach_gate = gate.clone();
        let attach_txs = txs.clone();
        let attach_replay = replay.clone();
        let attach_start = start.clone();
        let attach_thread = thread::spawn(move || {
            attach_start.wait();
            attach_sink_atomic(
                &attach_gate,
                &attach_txs,
                &attach_replay,
                AudioSink::Usb,
                sink_tx,
            )
            .unwrap();
        });
        start.wait();
        event_thread.join().unwrap();
        attach_thread.join().unwrap();
        let matches = std::iter::from_fn(|| sink_rx.try_recv().ok())
            .filter(|event| {
                matches!(
                    event,
                    EngineEvent::SetMasterVolume { volume_pct } if *volume_pct == 72.0
                )
            })
            .count();
        assert_eq!(matches, 1);
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    #[test]
    fn orange_audio_fanout_and_replay_cover_a_late_uac_sink() {
        let (dac_tx, mut dac_rx) = event_queue();
        let (usb_tx, mut usb_rx) = event_queue();
        let sinks = Arc::new(Mutex::new(Vec::new()));
        register_sink(&sinks, AudioSink::Jack, dac_tx);
        register_sink(&sinks, AudioSink::Usb, usb_tx);

        let event = EngineEvent::SetMasterVolume { volume_pct: 72.0 };
        broadcast_event(&sinks, event.clone()).unwrap();
        assert!(
            matches!(dac_rx.try_recv(), Ok(EngineEvent::SetMasterVolume { volume_pct }) if volume_pct == 72.0)
        );
        assert!(
            matches!(usb_rx.try_recv(), Ok(EngineEvent::SetMasterVolume { volume_pct }) if volume_pct == 72.0)
        );

        let mut cache = ReplayCache::default();
        cache.remember(&event);
        let (late_tx, mut late_rx) = event_queue();
        replay_to_sink(&late_tx, &Arc::new(Mutex::new(cache))).unwrap();
        assert!(matches!(
            late_rx.try_recv(),
            Ok(EngineEvent::SetPreparedInstruments(_))
        ));
        assert!(
            matches!(late_rx.try_recv(), Ok(EngineEvent::SetMasterVolume { volume_pct }) if volume_pct == 72.0)
        );
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    #[test]
    fn orange_uac_sink_loss_does_not_fail_required_dac_fanout() {
        let (dac_tx, mut dac_rx) = event_queue();
        let (usb_tx, usb_rx) = event_queue();
        let sinks = Arc::new(Mutex::new(Vec::new()));
        register_sink(&sinks, AudioSink::Jack, dac_tx);
        register_sink(&sinks, AudioSink::Usb, usb_tx);
        drop(usb_rx);

        assert!(broadcast_event(&sinks, EngineEvent::AllNotesOff).is_ok());
        assert!(matches!(dac_rx.try_recv(), Ok(EngineEvent::AllNotesOff)));
        assert_eq!(sinks.lock().unwrap().len(), 1);
    }
}
