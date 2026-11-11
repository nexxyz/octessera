use super::spawn_audio_engine_thread;
use rodio_engine_source::{event_queue, EngineEvent};

#[test]
fn no_audio_keeps_the_engine_taking_events() {
    let (engine_tx, engine_rx) = event_queue();
    let (load_tx, _load_rx) = rodio_engine_source::audio_load_status_channel();
    let (failure_tx, failure_rx) = std::sync::mpsc::channel();

    spawn_audio_engine_thread(engine_rx, load_tx, failure_tx, true, Default::default());

    assert!(engine_tx.send(EngineEvent::AllNotesOff).is_ok());
    assert!(failure_rx.try_recv().is_err());
}
