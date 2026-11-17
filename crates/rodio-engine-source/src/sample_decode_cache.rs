use realtime_engine::synth::SampleBuffer;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// Decoded samples the cache keeps beyond the ones still in use, so browsing previews does
/// not grow memory without bound.
const SAMPLE_CACHE_BUDGET_BYTES: u64 = 64 * 1024 * 1024;

/// Decoded samples by path, shared by the hosts' audio preparation. An entry is reused only
/// while the file keeps its length and modification time, and the least recently used entries
/// nothing else holds are dropped once the cache outgrows its budget. Playing voices keep
/// their own reference, so dropping an entry never cuts a sound short.
#[derive(Clone)]
pub struct SampleDecodeCache {
    cache: Arc<Mutex<CachedSamples>>,
    budget_bytes: u64,
}

#[derive(Debug)]
pub enum SampleDecodeCacheError {
    LookupLock,
    InsertionLock(SampleBuffer),
}

#[derive(Default)]
struct CachedSamples {
    entries: HashMap<String, CachedSample>,
    uses: u64,
}

struct CachedSample {
    stamp: FileStamp,
    buffer: SampleBuffer,
    last_use: u64,
}

type FileStamp = (u64, Option<SystemTime>);

impl Default for SampleDecodeCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SampleDecodeCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(CachedSamples::default())),
            budget_bytes: SAMPLE_CACHE_BUDGET_BYTES,
        }
    }

    pub fn load(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<Option<SampleBuffer>, SampleDecodeCacheError> {
        let path = path.as_ref();
        let key = path.to_string_lossy().into_owned();
        let stamp = std::fs::metadata(path)
            .ok()
            .map(|metadata| (metadata.len(), metadata.modified().ok()));
        match self.cache.lock() {
            Ok(mut cache) => {
                if let Some(buffer) = stamp.and_then(|stamp| cache.reuse(&key, stamp)) {
                    return Ok(Some(buffer));
                }
            }
            Err(_) => return Err(SampleDecodeCacheError::LookupLock),
        }
        let (Some(stamp), Some(buffer)) = (stamp, crate::decode_sample_file(path)) else {
            return Ok(None);
        };
        let Ok(mut cache) = self.cache.lock() else {
            return Err(SampleDecodeCacheError::InsertionLock(buffer));
        };
        cache.insert(key, stamp, buffer.clone(), self.budget_bytes);
        Ok(Some(buffer))
    }
}

impl CachedSamples {
    fn reuse(&mut self, key: &str, stamp: FileStamp) -> Option<SampleBuffer> {
        self.uses += 1;
        let entry = self
            .entries
            .get_mut(key)
            .filter(|entry| entry.stamp == stamp)?;
        entry.last_use = self.uses;
        Some(entry.buffer.clone())
    }

    fn insert(&mut self, key: String, stamp: FileStamp, buffer: SampleBuffer, budget_bytes: u64) {
        self.uses += 1;
        self.entries.insert(
            key,
            CachedSample {
                stamp,
                buffer,
                last_use: self.uses,
            },
        );
        let mut bytes = self.entries.values().map(CachedSample::bytes).sum::<u64>();
        while bytes > budget_bytes {
            let Some(unused) = self
                .entries
                .iter()
                .filter(|(_, entry)| Arc::strong_count(&entry.buffer.samples) == 1)
                .min_by_key(|(_, entry)| entry.last_use)
                .map(|(key, _)| key.clone())
            else {
                return;
            };
            if let Some(entry) = self.entries.remove(&unused) {
                bytes -= entry.bytes();
            }
        }
    }
}

impl CachedSample {
    fn bytes(&self) -> u64 {
        std::mem::size_of_val(&*self.buffer.samples) as u64
    }
}

#[cfg(test)]
mod tests;
