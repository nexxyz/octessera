use super::*;

#[test]
fn inline_source_publishes_the_exact_final_jack_block_once() {
    let (tx, rx) = event_queue();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 1_000,
    })
    .unwrap();
    let pair = new_pcm_mirror();
    let producer = pair.producer;
    let mut mirror = pair.consumer;
    let mut source = EngineSource::with_block_frames(rx, 44_100, 32);
    source.set_pcm_mirror_producers([Some(producer), None]);

    let jack: Vec<u32> = (0..64).map(|_| source.next().unwrap().to_bits()).collect();
    assert!(mirror.begin_callback());
    let mirrored: Vec<u32> = (0..64)
        .map(|_| mirror.next_sample().unwrap().to_bits())
        .collect();

    assert_eq!(mirrored, jack);
    assert!(!mirror.begin_callback());
}

#[test]
fn dropping_source_invalidates_its_pcm_mirrors_before_shutdown_handoff() {
    let (_tx, rx) = event_queue();
    let pair = new_pcm_mirror();
    let mut mirror = pair.consumer;
    let mut source = EngineSource::new(rx, 44_100);
    source.set_pcm_mirror_producers([Some(pair.producer), None]);

    drop(source);

    assert!(!mirror.begin_callback());
}
