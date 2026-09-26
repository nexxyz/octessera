use super::*;

fn frame(revision: u64, pixels: Vec<u8>) -> RunnerMessage {
    RunnerMessage::OledFrame {
        revision,
        width: 128,
        height: 128,
        format: "rgb565be".into(),
        pixels,
    }
}

#[test]
fn candidate_is_black_until_matching_snapshot_promotes_it() {
    let bytes = vec![7; OLED_FRAME_BYTES];
    let mut cache = OledFrameCache::default();
    cache.ingest(&frame(1, bytes.clone()));
    assert_eq!(cache.accepted_pixels, None);
    assert_eq!(cache.candidate_revision, 1);
    assert_eq!(cache.accept_reference(Some(1)), Some(bytes.as_slice()));
    assert_eq!(cache.accepted_revision, 1);
    assert_eq!(cache.fault(), None);
}

#[test]
fn accepted_frame_clones_share_immutable_pixels() {
    let bytes = vec![7; OLED_FRAME_BYTES];
    let mut cache = OledFrameCache::default();
    cache.ingest(&RunnerMessage::OledFrame {
        revision: 1,
        width: 128,
        height: 128,
        format: "rgb565be".into(),
        pixels: bytes,
    });
    cache.accept_reference(Some(1));

    let first = cache.accepted_frame().unwrap();
    let second = cache.accepted_frame().unwrap();
    assert_eq!(first.revision(), 1);
    assert!(Arc::ptr_eq(&first.pixels, &second.pixels));
}

#[test]
fn publication_requires_exact_positive_snapshot_revision() {
    let bytes = vec![7; OLED_FRAME_BYTES];
    let mut cache = OledFrameCache::default();
    cache.ingest(&frame(1, bytes.clone()));
    assert!(cache
        .publication_for_snapshot(&serde_json::json!({"oledFrameRevision": 1}), true)
        .is_err());

    cache.accept_reference(Some(1));
    let publication = cache
        .publication_for_snapshot(&serde_json::json!({"oledFrameRevision": 1}), true)
        .unwrap();
    assert_eq!(
        publication,
        OledFramePublication::Native(cache.accepted_frame().unwrap())
    );
    assert!(cache
        .publication_for_snapshot(&serde_json::json!({"oledFrameRevision": 2}), true)
        .is_err());
    assert_eq!(
        cache
            .publication_for_snapshot(&serde_json::json!({"oledFrameRevision": 2}), false)
            .unwrap(),
        OledFramePublication::RetainedLastGood(cache.accepted_frame().unwrap())
    );
    assert_eq!(publication.pixels(), Some(bytes.as_slice()));
}

#[test]
fn initial_handoff_only_accepts_exact_accepted_native_pair() {
    let bytes = vec![7; OLED_FRAME_BYTES];
    let mut cache = OledFrameCache::default();
    cache.ingest(&frame(1, bytes.clone()));
    assert!(cache
        .publication_for_snapshot(&serde_json::json!({"oledFrameRevision": 1}), true)
        .is_err());

    cache.accept_reference(Some(1));
    assert!(cache
        .publication_for_snapshot(&serde_json::json!({"oledFrameRevision": 2}), true)
        .is_err());
    assert_eq!(
        cache.publication_for_snapshot(&serde_json::json!({"oledFrameRevision": 1}), true),
        Ok(OledFramePublication::Native(
            cache.accepted_frame().unwrap()
        ))
    );

    cache.ingest(&frame(1, vec![8; OLED_FRAME_BYTES]));
    assert_eq!(cache.fault(), Some(OledFrameCacheFault::Conflict));
    assert!(cache
        .publication_for_snapshot(&serde_json::json!({"oledFrameRevision": 1}), true)
        .is_err());
}

#[test]
fn cache_fault_publishes_retained_last_good_bytes() {
    let accepted = vec![9; OLED_FRAME_BYTES];
    let mut cache = OledFrameCache::default();
    cache.ingest(&frame(1, accepted.clone()));
    cache.accept_reference(Some(1));
    cache.ingest(&frame(1, vec![8; OLED_FRAME_BYTES]));

    assert_eq!(cache.accepted_pixels.as_deref(), Some(accepted.as_slice()));
    assert_eq!(
        cache
            .publication_for_snapshot(&serde_json::json!({"oledFrameRevision": 1}), false)
            .unwrap(),
        OledFramePublication::RetainedLastGood(cache.accepted_frame().unwrap())
    );
}

#[test]
fn newer_candidate_never_replaces_accepted_bytes_before_reference() {
    let old = vec![1; OLED_FRAME_BYTES];
    let newer = vec![2; OLED_FRAME_BYTES];
    let mut cache = OledFrameCache::default();
    cache.ingest(&frame(1, old.clone()));
    cache.accept_reference(Some(1));
    cache.ingest(&frame(2, newer.clone()));
    assert_eq!(cache.accepted_pixels.as_deref(), Some(old.as_slice()));
    assert_eq!(cache.accept_reference(Some(1)), Some(old.as_slice()));
    assert_eq!(cache.accepted_pixels.as_deref(), Some(old.as_slice()));
    assert_eq!(cache.accept_reference(Some(2)), Some(newer.as_slice()));
}

#[test]
fn duplicate_conflict_malformed_zero_wrong_fields_and_recovery_are_typed() {
    let bytes = vec![3; OLED_FRAME_BYTES];
    let mut cache = OledFrameCache::default();
    cache.ingest(&frame(1, bytes.clone()));
    cache.ingest(&frame(1, vec![4; OLED_FRAME_BYTES]));
    assert_eq!(cache.fault(), Some(OledFrameCacheFault::Conflict));
    cache.ingest(&RunnerMessage::OledFrame {
        revision: 0,
        width: 128,
        height: 128,
        format: "rgb565be".into(),
        pixels: bytes.clone(),
    });
    assert_eq!(cache.fault(), Some(OledFrameCacheFault::Conflict));
    cache.ingest(&RunnerMessage::OledFrame {
        revision: 2,
        width: 127,
        height: 128,
        format: "rgb565be".into(),
        pixels: vec![0; OLED_FRAME_BYTES],
    });
    assert_eq!(cache.fault(), Some(OledFrameCacheFault::Conflict));
    cache.accept_reference(Some(1));
    assert_eq!(cache.fault(), Some(OledFrameCacheFault::Conflict));
    cache.ingest(&frame(2, vec![5; OLED_FRAME_BYTES]));
    assert_eq!(
        cache.accept_reference(Some(2)),
        Some(&[5; OLED_FRAME_BYTES][..])
    );
    assert_eq!(cache.fault(), None);
}

#[test]
fn candidate_conflict_is_invalidated_and_recovers_only_with_newer_pair() {
    let mut cache = OledFrameCache::default();
    cache.ingest(&frame(1, vec![1; OLED_FRAME_BYTES]));
    cache.ingest(&frame(1, vec![2; OLED_FRAME_BYTES]));
    assert_eq!(cache.candidate_revision, 0);
    assert_eq!(cache.fault(), Some(OledFrameCacheFault::Conflict));
    cache.ingest(&frame(1, vec![1; OLED_FRAME_BYTES]));
    assert_eq!(cache.candidate_revision, 0);
    assert_eq!(cache.fault(), Some(OledFrameCacheFault::Conflict));

    cache.ingest(&frame(2, vec![3; OLED_FRAME_BYTES]));
    assert_eq!(
        cache.accept_reference(Some(2)),
        Some(&[3; OLED_FRAME_BYTES][..])
    );
    assert_eq!(cache.fault(), None);
}

#[test]
fn accepted_conflict_stays_sticky_across_exact_reference() {
    let bytes = vec![1; OLED_FRAME_BYTES];
    let mut cache = OledFrameCache::default();
    cache.ingest(&frame(1, bytes.clone()));
    cache.accept_reference(Some(1));
    cache.ingest(&frame(1, vec![2; OLED_FRAME_BYTES]));
    assert_eq!(cache.fault(), Some(OledFrameCacheFault::Conflict));
    assert_eq!(cache.accept_reference(Some(1)), Some(bytes.as_slice()));
    assert_eq!(cache.fault(), Some(OledFrameCacheFault::Conflict));
    cache.ingest(&frame(2, vec![3; OLED_FRAME_BYTES]));
    cache.accept_reference(Some(2));
    assert_eq!(cache.fault(), None);
}

#[test]
fn stale_missing_and_future_references_retain_last_accepted_pair() {
    let old = vec![9; OLED_FRAME_BYTES];
    let mut cache = OledFrameCache::default();
    cache.ingest(&frame(1, old.clone()));
    cache.accept_reference(Some(1));
    cache.ingest(&frame(3, vec![3; OLED_FRAME_BYTES]));
    assert_eq!(cache.accept_reference(Some(2)), Some(old.as_slice()));
    assert_eq!(cache.fault(), Some(OledFrameCacheFault::Future));
    assert_eq!(cache.accept_reference(Some(0)), Some(old.as_slice()));
    assert_eq!(cache.fault(), Some(OledFrameCacheFault::Stale));
    assert_eq!(cache.accept_reference(None), Some(old.as_slice()));
    assert_eq!(cache.fault(), Some(OledFrameCacheFault::Missing));
    cache.ingest(&frame(2, vec![2; OLED_FRAME_BYTES]));
    assert_eq!(cache.candidate_revision, 3);
    cache.ingest(&frame(3, vec![3; OLED_FRAME_BYTES]));
    assert_eq!(
        cache.accept_reference(Some(3)),
        Some(&[3; OLED_FRAME_BYTES][..])
    );
}
