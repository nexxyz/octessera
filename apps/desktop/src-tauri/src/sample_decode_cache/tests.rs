use super::*;
use std::sync::{mpsc, Arc};
use std::time::Duration;

#[test]
fn sample_decode_cache_reuses_resolved_sample_across_clones() {
    let prep_cache = SampleDecodeCache::new();
    let host_cache = prep_cache.clone();
    let resolved_path = crate::samples::resolve_sample_file("samples/Drum/kick/Kick2.wav")
        .expect("bundled sample resolves");

    let prep_buffer = prep_cache
        .load(&resolved_path)
        .expect("sample cache lookup")
        .expect("bundled sample decodes");
    let host_buffer = host_cache
        .load(&resolved_path)
        .expect("cached sample lookup")
        .expect("cached sample loads");

    assert!(Arc::ptr_eq(&prep_buffer.samples, &host_buffer.samples));
    assert!(prep_cache
        .cache
        .lock()
        .unwrap()
        .contains_key(&resolved_path));
}

#[test]
fn sample_decode_cache_does_not_cache_decode_failures() {
    let cache = SampleDecodeCache::new();
    let missing_path = std::env::temp_dir().join(format!(
        "octessera-missing-sample-{}-{}.wav",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let missing_path = missing_path.to_string_lossy().into_owned();

    assert!(matches!(cache.load(&missing_path), Ok(None)));
    assert!(cache.cache.lock().unwrap().is_empty());
}

#[test]
fn sample_decode_cache_keeps_lookup_lock_failure_distinct() {
    let cache = SampleDecodeCache::new();
    let poisoned_cache = cache.clone();
    let poison_handle = std::thread::spawn(move || {
        let _guard = poisoned_cache.cache.lock().unwrap();
        panic!("poison sample cache for lock failure test");
    });
    assert!(poison_handle.join().is_err());

    assert!(matches!(
        cache.load("missing-sample.wav"),
        Err(SampleDecodeCacheError::LookupLock)
    ));
}

#[test]
fn sample_decode_cache_concurrent_clones_complete_and_retain_entry() {
    let cache = SampleDecodeCache::new();
    let resolved_path = crate::samples::resolve_sample_file("samples/Drum/kick/Kick2.wav")
        .expect("bundled sample resolves");
    let (result_tx, result_rx) = mpsc::channel();
    let mut handles = Vec::new();

    for _ in 0..4 {
        let cache = cache.clone();
        let resolved_path = resolved_path.clone();
        let result_tx = result_tx.clone();
        handles.push(std::thread::spawn(move || {
            result_tx
                .send(cache.load(&resolved_path))
                .expect("cache load result receiver");
        }));
    }
    drop(result_tx);

    for _ in 0..4 {
        let result = result_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("concurrent cache load completed")
            .expect("cache lookup");
        assert!(result.is_some());
    }
    for handle in handles {
        handle.join().expect("cache load thread completed");
    }

    assert!(cache
        .load(&resolved_path)
        .expect("eventual cache lookup")
        .is_some());
    assert!(cache.cache.lock().unwrap().contains_key(&resolved_path));
}
