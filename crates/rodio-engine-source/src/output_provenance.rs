#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(any(test, feature = "routing-tree-executor"))]
pub(crate) enum PersistentOutputKind {
    Fresh,
    Repeated,
    DroppedSilence,
    FatalSilence,
}

#[cfg(feature = "output-provenance")]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PersistentOutputProvenanceSnapshot {
    pub repeated_quantum_incidents: u64,
    pub repeated_pcm_frames: u64,
    pub silent_quantum_incidents: u64,
    pub silent_pcm_frames: u64,
}

#[cfg(feature = "output-provenance")]
#[derive(Default)]
pub(super) struct PersistentOutputProvenance {
    kind: Option<PersistentOutputKind>,
    frames_remaining: usize,
    incident_counted: bool,
    snapshot: PersistentOutputProvenanceSnapshot,
}

#[cfg(feature = "output-provenance")]
impl PersistentOutputProvenance {
    pub(super) fn start_block(&mut self, kind: PersistentOutputKind, frames: usize) {
        self.kind = Some(kind);
        self.frames_remaining = frames;
        self.incident_counted = false;
    }

    pub(super) fn rebase(&mut self) {
        self.snapshot = PersistentOutputProvenanceSnapshot::default();
        if self.frames_remaining > 0 {
            self.incident_counted = false;
        }
    }

    pub(super) fn consume_frame(&mut self) {
        let Some(kind) = self.kind else {
            return;
        };
        if self.frames_remaining == 0 {
            self.kind = None;
            return;
        }
        if !self.incident_counted {
            match kind {
                PersistentOutputKind::Fresh => {}
                PersistentOutputKind::Repeated => {
                    self.snapshot.repeated_quantum_incidents =
                        self.snapshot.repeated_quantum_incidents.saturating_add(1);
                }
                PersistentOutputKind::DroppedSilence | PersistentOutputKind::FatalSilence => {
                    self.snapshot.silent_quantum_incidents =
                        self.snapshot.silent_quantum_incidents.saturating_add(1);
                }
            }
            self.incident_counted = true;
        }
        match kind {
            PersistentOutputKind::Fresh => {}
            PersistentOutputKind::Repeated => {
                self.snapshot.repeated_pcm_frames =
                    self.snapshot.repeated_pcm_frames.saturating_add(1);
            }
            PersistentOutputKind::DroppedSilence | PersistentOutputKind::FatalSilence => {
                self.snapshot.silent_pcm_frames = self.snapshot.silent_pcm_frames.saturating_add(1);
            }
        }
        self.frames_remaining -= 1;
        if self.frames_remaining == 0 {
            self.kind = None;
        }
    }

    pub(super) fn snapshot(&self) -> PersistentOutputProvenanceSnapshot {
        self.snapshot
    }
}
