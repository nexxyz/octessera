use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub(crate) struct SampleDecodeCache {
    cache: Arc<Mutex<HashMap<String, realtime_engine::synth::SampleBuffer>>>,
}

#[derive(Debug)]
pub(crate) enum SampleDecodeCacheError {
    LookupLock,
    InsertionLock(realtime_engine::synth::SampleBuffer),
}

impl SampleDecodeCache {
    pub(crate) fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) fn load(
        &self,
        resolved_path: &str,
    ) -> Result<Option<realtime_engine::synth::SampleBuffer>, SampleDecodeCacheError> {
        let cached = match self.cache.lock() {
            Ok(cache) => cache.get(resolved_path).cloned(),
            Err(_) => return Err(SampleDecodeCacheError::LookupLock),
        };
        if let Some(buffer) = cached {
            return Ok(Some(buffer));
        }

        let Some(buffer) = rodio_engine_source::decode_sample_file(resolved_path) else {
            return Ok(None);
        };
        let Ok(mut cache) = self.cache.lock() else {
            return Err(SampleDecodeCacheError::InsertionLock(buffer));
        };
        cache.insert(resolved_path.to_string(), buffer.clone());
        Ok(Some(buffer))
    }
}

#[cfg(test)]
#[path = "sample_decode_cache_tests.rs"]
mod tests;
