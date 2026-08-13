use playback_runtime::oled_frame::OLED_FRAME_BYTES;
use playback_runtime::RunnerMessage;
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AcceptedOledFrame {
    revision: u64,
    pixels: Arc<[u8]>,
}

impl AcceptedOledFrame {
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum OledFramePublication {
    Native(AcceptedOledFrame),
    RetainedLastGood(AcceptedOledFrame),
    ExplicitBlack,
}

impl OledFramePublication {
    pub(crate) fn is_native(&self) -> bool {
        matches!(self, Self::Native(_))
    }

    pub(crate) fn revision(&self) -> Option<u64> {
        match self {
            Self::Native(frame) | Self::RetainedLastGood(frame) => Some(frame.revision()),
            Self::ExplicitBlack => None,
        }
    }

    pub(crate) fn key(&self) -> OledFrameKey {
        match self {
            Self::Native(frame) | Self::RetainedLastGood(frame) => {
                OledFrameKey::Native(frame.revision())
            }
            Self::ExplicitBlack => OledFrameKey::ExplicitBlack,
        }
    }

    pub(crate) fn pixels(&self) -> Option<&[u8]> {
        match self {
            Self::Native(frame) | Self::RetainedLastGood(frame) => Some(frame.pixels()),
            Self::ExplicitBlack => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn test_native(revision: u64, pixels: Vec<u8>) -> Self {
        Self::Native(AcceptedOledFrame {
            revision,
            pixels: Arc::from(pixels),
        })
    }

    #[cfg(test)]
    pub(crate) fn test_retained_last_good(revision: u64, pixels: Vec<u8>) -> Self {
        Self::RetainedLastGood(AcceptedOledFrame {
            revision,
            pixels: Arc::from(pixels),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OledFrameKey {
    Native(u64),
    ExplicitBlack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OledFrameCacheFault {
    Malformed,
    Conflict,
    Missing,
    Future,
    Stale,
}

impl OledFrameCacheFault {
    pub(crate) fn into_runtime_fault(self) -> playback_runtime::RuntimeOledCacheFault {
        match self {
            Self::Malformed => playback_runtime::RuntimeOledCacheFault::Malformed,
            Self::Conflict => playback_runtime::RuntimeOledCacheFault::Conflict,
            Self::Missing => playback_runtime::RuntimeOledCacheFault::Missing,
            Self::Future => playback_runtime::RuntimeOledCacheFault::Future,
            Self::Stale => playback_runtime::RuntimeOledCacheFault::Stale,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct OledFrameCache {
    accepted_revision: u64,
    accepted_pixels: Option<Arc<[u8]>>,
    candidate_revision: u64,
    candidate_pixels: Option<Arc<[u8]>>,
    blocked_candidate_revision: u64,
    fault: Option<OledFrameCacheFault>,
}

impl OledFrameCache {
    pub(crate) fn ingest(&mut self, message: &RunnerMessage) {
        let RunnerMessage::OledFrame {
            revision,
            width,
            height,
            format,
            pixels,
        } = message
        else {
            return;
        };
        if *width != 128 || *height != 128 || format != "rgb565be" || *revision == 0 {
            self.set_fault(OledFrameCacheFault::Malformed);
            return;
        }
        if pixels.len() != OLED_FRAME_BYTES {
            self.set_fault(OledFrameCacheFault::Malformed);
            return;
        }
        if *revision <= self.accepted_revision {
            if *revision == self.accepted_revision
                && self.accepted_pixels.as_deref() != Some(pixels.as_slice())
            {
                self.set_fault(OledFrameCacheFault::Conflict);
            } else if *revision < self.accepted_revision {
                self.set_fault(OledFrameCacheFault::Stale);
            }
            return;
        }
        if *revision <= self.blocked_candidate_revision {
            self.set_fault(OledFrameCacheFault::Conflict);
            return;
        }
        if *revision == self.candidate_revision {
            if self.candidate_pixels.as_deref() != Some(pixels.as_slice()) {
                self.set_fault(OledFrameCacheFault::Conflict);
                self.blocked_candidate_revision = *revision;
                self.candidate_revision = 0;
                self.candidate_pixels = None;
            }
            return;
        }
        if *revision < self.candidate_revision {
            self.set_fault(OledFrameCacheFault::Stale);
            return;
        }
        self.candidate_revision = *revision;
        self.candidate_pixels = Some(Arc::from(pixels.clone()));
    }

    #[cfg(test)]
    pub(crate) fn accept_reference(&mut self, revision: Option<u64>) -> Option<&[u8]> {
        self.accept_reference_result(Ok(revision))
    }

    pub(crate) fn accept_reference_value(&mut self, snapshot: &Value) -> Option<&[u8]> {
        let revision = match snapshot.get("oledFrameRevision") {
            None => Ok(None),
            Some(value) => value
                .as_u64()
                .filter(|revision| *revision > 0)
                .map(|revision| Ok(Some(revision)))
                .unwrap_or(Err(())),
        };
        self.accept_reference_result(revision)
    }

    fn accept_reference_result(&mut self, revision: Result<Option<u64>, ()>) -> Option<&[u8]> {
        let revision = match revision {
            Ok(revision) => revision,
            Err(()) => {
                self.set_fault(OledFrameCacheFault::Malformed);
                return self.accepted_pixels.as_deref();
            }
        };
        match revision {
            None => {
                if self.accepted_revision > 0 {
                    self.set_fault(OledFrameCacheFault::Missing);
                }
            }
            Some(revision) if revision == self.accepted_revision && revision > 0 => {
                if self.fault != Some(OledFrameCacheFault::Conflict)
                    || self.accepted_pixels.is_none()
                {
                    self.fault = None;
                }
            }
            Some(revision)
                if revision == self.candidate_revision
                    && revision > self.accepted_revision
                    && revision > 0 =>
            {
                self.accepted_revision = revision;
                self.accepted_pixels = self.candidate_pixels.take();
                self.candidate_revision = 0;
                self.blocked_candidate_revision = 0;
                self.fault = None;
            }
            Some(revision) if revision > self.accepted_revision => {
                self.set_fault(OledFrameCacheFault::Future);
            }
            Some(_) => {
                self.set_fault(OledFrameCacheFault::Stale);
            }
        }
        self.accepted_pixels.as_deref()
    }

    pub(crate) fn accepted_frame(&self) -> Option<AcceptedOledFrame> {
        Some(AcceptedOledFrame {
            revision: self.accepted_revision,
            pixels: self.accepted_pixels.clone()?,
        })
    }

    pub(crate) fn publication_for_snapshot(
        &mut self,
        snapshot: &Value,
        initial: bool,
    ) -> Result<OledFramePublication, String> {
        let required_revision = match snapshot.get("oledFrameRevision") {
            None => {
                if self.accepted_revision > 0 {
                    self.set_fault(OledFrameCacheFault::Missing);
                }
                None
            }
            Some(value) => match value.as_u64().filter(|revision| *revision > 0) {
                Some(revision) => Some(revision),
                None => {
                    self.set_fault(OledFrameCacheFault::Malformed);
                    None
                }
            },
        };
        let matching = self
            .accepted_frame()
            .filter(|frame| Some(frame.revision()) == required_revision)
            .filter(|_| self.fault.is_none());
        if let Some(frame) = matching {
            return Ok(OledFramePublication::Native(frame));
        }
        if initial {
            Err("OLED initial snapshot has no accepted matching native frame".into())
        } else if let Some(frame) = self.accepted_frame() {
            Ok(OledFramePublication::RetainedLastGood(frame))
        } else {
            Ok(OledFramePublication::ExplicitBlack)
        }
    }

    fn set_fault(&mut self, fault: OledFrameCacheFault) {
        if fault == OledFrameCacheFault::Conflict
            || self.fault != Some(OledFrameCacheFault::Conflict)
        {
            self.fault = Some(fault);
        }
    }

    pub(crate) fn fault(&self) -> Option<OledFrameCacheFault> {
        self.fault
    }
}

#[cfg(test)]
mod tests {
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
        cache.ingest(&frame(1, bytes));
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
}
