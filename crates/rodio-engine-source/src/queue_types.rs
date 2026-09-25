use crate::EngineEvent;
use std::fmt::{Display, Formatter};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QueueEventClass {
    Emergency,
    Musical,
    StructuralBarrier,
    StructuralNonRetiring,
    StructuralRetiring,
}

impl QueueEventClass {
    pub(crate) fn from_event(event: &EngineEvent) -> Self {
        if is_musical_event(event) {
            return Self::Musical;
        }
        if matches!(event, EngineEvent::ProbeMark { .. }) {
            return Self::StructuralBarrier;
        }
        if is_retirement_producing_event(event) {
            Self::StructuralRetiring
        } else {
            Self::StructuralNonRetiring
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueSendError {
    Full { queue: QueueKind },
    Disconnected { queue: QueueKind },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueKind {
    Musical,
    Structural,
    Latest,
    Preview,
    Emergency,
}

impl QueueSendError {
    pub(crate) fn full(queue: QueueKind) -> Self {
        Self::Full { queue }
    }

    pub(crate) fn disconnected(queue: QueueKind) -> Self {
        Self::Disconnected { queue }
    }

    pub fn is_full(&self) -> bool {
        matches!(self, Self::Full { .. })
    }

    pub fn is_musical_full(&self) -> bool {
        matches!(
            self,
            Self::Full {
                queue: QueueKind::Musical
            }
        )
    }

    pub fn is_structural_full(&self) -> bool {
        matches!(
            self,
            Self::Full {
                queue: QueueKind::Structural
            }
        )
    }

    pub fn is_ordered_full(&self) -> bool {
        self.is_musical_full()
    }
}

impl Display for QueueSendError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full {
                queue: QueueKind::Musical,
            } => f.write_str("musical audio event queue is full"),
            Self::Full {
                queue: QueueKind::Structural,
            } => f.write_str("structural audio event queue is full"),
            Self::Full { queue } => write!(f, "{queue:?} audio event lane is full"),
            Self::Disconnected { queue } => write!(f, "{queue:?} audio event lane disconnected"),
        }
    }
}

impl std::error::Error for QueueSendError {}

pub(crate) fn is_latest_event(event: &EngineEvent) -> bool {
    matches!(
        event,
        EngineEvent::SetMasterVolume { .. }
            | EngineEvent::SetDspConfig { .. }
            | EngineEvent::SetVoiceStealingMode { .. }
            | EngineEvent::SetInstrumentMixer { .. }
            | EngineEvent::SetFxBusMixer { .. }
            | EngineEvent::SetSynthParam { .. }
            | EngineEvent::SetFmParam { .. }
            | EngineEvent::SetPluckParam { .. }
            | EngineEvent::SetDrumParam { .. }
            | EngineEvent::SetSampleBankParam { .. }
            | EngineEvent::SetFxBusParam { .. }
            | EngineEvent::SetGlobalFxParam { .. }
            | EngineEvent::MomentaryFxUpdate(_)
    )
}

pub(crate) fn is_musical_event(event: &EngineEvent) -> bool {
    matches!(
        event,
        EngineEvent::NoteOn { .. }
            | EngineEvent::DrumHit { .. }
            | EngineEvent::NoteOff { .. }
            | EngineEvent::Cc { .. }
    )
}

fn is_retirement_producing_event(event: &EngineEvent) -> bool {
    matches!(
        event,
        EngineEvent::SetPreparedInstruments { .. }
            | EngineEvent::SetPreparedAudioConfig { .. }
            | EngineEvent::SetPreparedInstrumentOwner { .. }
            | EngineEvent::SetPreparedSampleBank { .. }
            | EngineEvent::SetPreparedFxBusSlot { .. }
            | EngineEvent::SetPreparedGlobalFxSlot { .. }
            | EngineEvent::PreparedMomentaryFxStart { .. }
            | EngineEvent::MomentaryFxStop { .. }
            | EngineEvent::PreviewSample { .. }
    )
}
