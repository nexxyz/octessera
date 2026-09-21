use crate::audio::AudioSink;
use crate::audio_replay::{is_replay_event, replay_to_sink, ReplayCache};
use rodio_engine_source::{EngineEvent, EngineEventSender, QueueKind, QueueSendError};
use std::sync::{Arc, Mutex};

pub(crate) type AudioAttachGate = Arc<Mutex<()>>;

pub(crate) fn new_attach_gate() -> AudioAttachGate {
    Arc::new(Mutex::new(()))
}

pub(crate) struct SinkSender {
    pub(crate) sink: AudioSink,
    tx: EngineEventSender,
}

pub(crate) enum AudioBroadcastError {
    Queue(QueueSendError),
    Registry(String),
}

impl std::fmt::Display for AudioBroadcastError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Queue(error) => error.fmt(formatter),
            Self::Registry(error) => formatter.write_str(error),
        }
    }
}

#[cfg(test)]
pub(crate) fn test_sink_sender(tx: EngineEventSender) -> SinkSender {
    test_sink_sender_for(AudioSink::Jack, tx)
}

#[cfg(test)]
pub(crate) fn test_sink_sender_for(sink: AudioSink, tx: EngineEventSender) -> SinkSender {
    SinkSender { sink, tx }
}

pub(crate) fn broadcast_event(
    txs: &Arc<Mutex<Vec<SinkSender>>>,
    event: EngineEvent,
) -> Result<(), AudioBroadcastError> {
    let mut disconnected = Vec::new();
    let mut first_error = None;
    let mut guard = txs
        .lock()
        .map_err(|_| AudioBroadcastError::Registry("audio sink registry lock failed".into()))?;
    for sink in guard.iter() {
        record_send_result(
            sink.sink,
            sink.tx.send(event.clone()),
            || sink.tx.send(EngineEvent::AllNotesOff),
            &mut first_error,
            &mut disconnected,
        );
    }
    guard.retain(|sink| !disconnected.contains(&sink.sink));
    first_error.map_or(Ok(()), Err)
}

fn remember_error(first_error: &mut Option<AudioBroadcastError>, error: QueueSendError) {
    first_error.get_or_insert(AudioBroadcastError::Queue(error));
}

fn record_send_result(
    sink: AudioSink,
    result: Result<(), QueueSendError>,
    send_emergency: impl FnOnce() -> Result<(), QueueSendError>,
    first_error: &mut Option<AudioBroadcastError>,
    disconnected: &mut Vec<AudioSink>,
) {
    match result {
        Ok(()) => {}
        Err(
            error @ QueueSendError::Full {
                queue: QueueKind::Latest,
            },
        ) => {
            if failed_sink_is_required(&sink) {
                remember_error(first_error, error);
            }
        }
        Err(error @ QueueSendError::Full { .. }) => {
            remember_error(first_error, error);
            match send_emergency() {
                Ok(()) => {}
                Err(QueueSendError::Disconnected { .. }) => disconnected.push(sink),
                Err(error) => remember_error(first_error, error),
            }
        }
        Err(error @ QueueSendError::Disconnected { .. }) => {
            if failed_sink_is_required(&sink) {
                remember_error(first_error, error);
            }
            disconnected.push(sink);
        }
    }
}

pub(crate) fn broadcast_event_atomic(
    gate: &AudioAttachGate,
    txs: &Arc<Mutex<Vec<SinkSender>>>,
    replay_events: &Arc<Mutex<ReplayCache>>,
    event: EngineEvent,
) -> Result<(), AudioBroadcastError> {
    let _gate = gate
        .lock()
        .map_err(|_| AudioBroadcastError::Registry("audio attach gate lock failed".into()))?;
    if is_replay_event(&event) {
        replay_events
            .lock()
            .map_err(|_| AudioBroadcastError::Registry("audio replay cache lock failed".into()))?
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

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(crate) fn remove_sink(txs: &Arc<Mutex<Vec<SinkSender>>>, sink: AudioSink) {
    if let Ok(mut txs) = txs.lock() {
        txs.retain(|entry| entry.sink != sink);
    }
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
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

#[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
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
#[path = "audio_sink_registry_tests.rs"]
mod tests;
