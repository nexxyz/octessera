use super::{next_block_deadline, spawn_audio_engine_thread};
use rodio_engine_source::{event_queue, EngineEvent};

#[test]
fn no_audio_keeps_the_engine_taking_events() {
    let (engine_tx, engine_rx) = event_queue();
    let (load_tx, _load_rx) = rodio_engine_source::audio_load_status_channel();
    let (failure_tx, failure_rx) = std::sync::mpsc::channel();

    spawn_audio_engine_thread(
        engine_rx,
        load_tx,
        failure_tx,
        std::sync::mpsc::channel().1,
        std::sync::mpsc::channel().0,
        true,
        Default::default(),
    );

    assert!(engine_tx.send(EngineEvent::AllNotesOff).is_ok());
    assert!(failure_rx.try_recv().is_err());
}

#[test]
fn no_audio_catches_up_a_short_lag_but_resyncs_after_a_stall() {
    let start = std::time::Instant::now();
    let block = std::time::Duration::from_millis(5);
    let ms = std::time::Duration::from_millis;

    assert_eq!(next_block_deadline(start, block, start), start + block);
    assert_eq!(
        next_block_deadline(start, block, start + ms(100)),
        start + block
    );
    assert_eq!(
        next_block_deadline(start, block, start + ms(500)),
        start + ms(500)
    );
}
