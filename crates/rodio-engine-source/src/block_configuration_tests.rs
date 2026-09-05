use super::*;

#[test]
fn explicit_profile_block_sizes_reach_source_configuration() {
    for block_frames in [64, 128, 256] {
        let (_tx, rx) = event_queue();
        let source = EngineSource::with_block_frames(rx, 44_100, block_frames);

        assert_eq!(source.block_frames(), block_frames);
    }
    let (_tx, rx) = event_queue();
    let source = EngineSource::with_block_frames(rx, 44_100, 1);
    assert_eq!(source.block_frames(), 32);
    assert_eq!(source.lookahead_frames(), 0);
}

#[test]
fn default_and_explicit_block_apis_use_inline_source_path() {
    let (default_tx, default_rx) = event_queue();
    let (explicit_tx, explicit_rx) = event_queue();
    let note_on = EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 1_000,
    };
    default_tx.send(note_on.clone()).unwrap();
    explicit_tx.send(note_on).unwrap();
    let mut default_source = EngineSource::new(default_rx, 44_100);
    let mut explicit_source =
        EngineSource::with_block_frames(explicit_rx, 44_100, DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES);
    assert_eq!(
        default_source.block_frames(),
        DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES
    );
    assert_eq!(
        explicit_source.block_frames(),
        DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES
    );

    for _ in 0..256 {
        assert_eq!(
            default_source.next().unwrap().to_bits(),
            explicit_source.next().unwrap().to_bits()
        );
    }
}

#[test]
fn explicit_block_size_respects_render_quantum_override_parser() {
    assert_eq!(resolve_audio_render_quantum_frames(Some("128"), 64), 128);
    assert_eq!(resolve_audio_render_quantum_frames(Some("invalid"), 64), 64);
    assert_eq!(resolve_audio_render_quantum_frames(Some("1"), 64), 32);
}

#[test]
fn inline_sources_report_zero_lookahead() {
    let (_tx, rx) = event_queue();
    let source = EngineSource::new(rx, 44_100);
    assert_eq!(source.lookahead_frames(), 0);
}
