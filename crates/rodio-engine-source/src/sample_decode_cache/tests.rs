use super::*;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, UNIX_EPOCH};

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "octessera-sample-cache-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn wav(&self, name: &str, samples: &[i16]) -> PathBuf {
        let path = self.0.join(name);
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 44_100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for sample in samples {
            writer.write_sample(*sample).unwrap();
        }
        writer.finalize().unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn cached(cache: &SampleDecodeCache, path: &Path) -> bool {
    cache
        .cache
        .lock()
        .unwrap()
        .entries
        .contains_key(path.to_string_lossy().as_ref())
}

#[test]
fn sample_decode_cache_reuses_a_sample_across_clones() {
    let dir = TempDir::new("reuse");
    let path = dir.wav("kick.wav", &[1, 2, 3]);
    let prep_cache = SampleDecodeCache::new();
    let host_cache = prep_cache.clone();

    let prep_buffer = prep_cache.load(&path).unwrap().unwrap();
    let host_buffer = host_cache.load(&path).unwrap().unwrap();

    assert!(Arc::ptr_eq(&prep_buffer.samples, &host_buffer.samples));
    assert!(cached(&prep_cache, &path));
}

#[test]
fn sample_decode_cache_does_not_cache_decode_failures() {
    let dir = TempDir::new("missing");
    let missing = dir.0.join("missing.wav");
    let cache = SampleDecodeCache::new();

    assert!(matches!(cache.load(&missing), Ok(None)));
    assert!(cache.cache.lock().unwrap().entries.is_empty());
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
    let dir = TempDir::new("concurrent");
    let path = dir.wav("kick.wav", &[1, 2, 3]);
    let cache = SampleDecodeCache::new();
    let (result_tx, result_rx) = mpsc::channel();
    let mut handles = Vec::new();

    for _ in 0..4 {
        let cache = cache.clone();
        let path = path.clone();
        let result_tx = result_tx.clone();
        handles.push(std::thread::spawn(move || {
            result_tx.send(cache.load(&path)).unwrap();
        }));
    }
    drop(result_tx);

    for _ in 0..4 {
        let result = result_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(result.unwrap().is_some());
    }
    for handle in handles {
        handle.join().unwrap();
    }
    assert!(cached(&cache, &path));
}

#[test]
fn a_file_overwritten_at_the_same_path_loads_its_new_audio() {
    let dir = TempDir::new("overwrite");
    let path = dir.wav("loop.wav", &[1, 2, 3]);
    let cache = SampleDecodeCache::new();
    let old = cache.load(&path).unwrap().unwrap();

    dir.wav("loop.wav", &[4, 5, 6, 7]);
    let new = cache.load(&path).unwrap().unwrap();

    assert_eq!(old.samples.len(), 3);
    assert_eq!(new.samples.len(), 4);
}

#[test]
fn the_least_recently_used_unheld_sample_leaves_a_full_cache() {
    let dir = TempDir::new("evict");
    let samples = [0_i16; 100];
    let first = dir.wav("first.wav", &samples);
    let second = dir.wav("second.wav", &samples);
    let held = dir.wav("held.wav", &samples);
    let third = dir.wav("third.wav", &samples);
    let mut cache = SampleDecodeCache::new();
    cache.budget_bytes = 3 * 100 * 4;

    let held_buffer = cache.load(&held).unwrap().unwrap();
    cache.load(&first).unwrap();
    cache.load(&second).unwrap();
    cache.load(&first).unwrap();
    cache.load(&third).unwrap();

    assert!(cached(&cache, &held));
    assert!(cached(&cache, &first));
    assert!(!cached(&cache, &second));
    assert!(cached(&cache, &third));
    assert_eq!(held_buffer.samples.len(), 100);
}

#[test]
fn a_sample_too_long_to_decode_on_the_boards_is_refused() {
    let dir = TempDir::new("oversize");
    let path = dir.0.join("long.wav");
    let samples = crate::sample_decode::MAX_DECODED_SAMPLE_BYTES as u32 / 4 + 1;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + samples).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&44_100_u32.to_le_bytes());
    bytes.extend_from_slice(&44_100_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&8_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&samples.to_le_bytes());
    bytes.resize(bytes.len() + samples as usize, 128);
    std::fs::write(&path, bytes).unwrap();
    let cache = SampleDecodeCache::new();

    assert!(matches!(cache.load(&path), Ok(None)));
    assert!(!cached(&cache, &path));
}
