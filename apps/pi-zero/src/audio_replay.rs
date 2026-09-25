use crate::audio::default_pi_instruments;
use realtime_engine::synth::{
    prepare_instruments_config, DrumParamId, FmParamId, FxParamId, PluckParamId, SampleBankConfig,
    SampleBankParamId, SynthParamId, DEFAULT_AUDIO_SAMPLE_RATE,
};
use rodio_engine_source::{EngineEvent, EngineEventSender};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub(crate) fn default_replay_events() -> ReplayCache {
    ReplayCache::default()
}

#[derive(Clone, Default)]
pub(crate) struct ReplayCache {
    audio_config: Option<ReplayValue>,
    sample_banks: Option<Vec<SampleBankConfig>>,
    full_generation: Option<u64>,
    keyed: BTreeMap<ReplayKey, ReplayValue>,
    sample_owner_generations: BTreeMap<usize, u64>,
}

#[derive(Clone)]
struct ReplayValue {
    generation: u64,
    event: EngineEvent,
}

impl ReplayCache {
    pub(crate) fn remember(&mut self, event: &EngineEvent) {
        if !is_replay_event(event) {
            return;
        }
        if let EngineEvent::SetPreparedAudioConfig { generation, config } = event {
            if self
                .full_generation
                .is_some_and(|current| *generation < current)
            {
                return;
            }
            let sample_banks = config
                .sample_banks()
                .map(<[SampleBankConfig]>::to_vec)
                .or_else(|| self.sample_banks.clone());
            if let Some(banks) = sample_banks.as_ref() {
                self.sample_banks = Some(banks.clone());
            }
            self.full_generation = Some(*generation);
            self.audio_config = Some(ReplayValue {
                generation: *generation,
                event: EngineEvent::SetPreparedAudioConfig {
                    generation: *generation,
                    config: config.with_sample_banks(sample_banks),
                },
            });
            self.keyed
                .retain(|_, value| value.generation >= *generation);
            self.sample_owner_generations
                .retain(|_, value| *value >= *generation);
            return;
        }
        if let EngineEvent::SetPreparedInstruments { .. } = event {
            if self.audio_config.is_none() {
                self.audio_config = Some(ReplayValue {
                    generation: 0,
                    event: event.clone(),
                });
            }
            return;
        }
        if let EngineEvent::SetPreparedInstrumentOwner {
            instrument_slot,
            generation,
            ..
        } = event
        {
            let slot = usize::from(*instrument_slot);
            if self
                .full_generation
                .is_some_and(|current| *generation < current)
                || self
                    .keyed
                    .get(&ReplayKey::InstrumentSlot(slot))
                    .is_some_and(|current| current.generation > *generation)
                || self
                    .sample_owner_generations
                    .get(&slot)
                    .is_some_and(|current| *current > *generation)
            {
                return;
            }
            if matches!(
                event,
                EngineEvent::SetPreparedInstrumentOwner {
                    sample_bank: Some(_),
                    ..
                }
            ) {
                self.keyed.remove(&ReplayKey::SampleBank(slot));
                self.sample_owner_generations.insert(slot, *generation);
            } else {
                self.preserve_atomic_sample_bank(slot);
            }
            self.keyed.insert(
                ReplayKey::InstrumentSlot(slot),
                ReplayValue {
                    generation: *generation,
                    event: event.clone(),
                },
            );
            return;
        }
        let Some(key) = replay_key(event) else {
            return;
        };
        let generation = event_generation(event);
        if self
            .full_generation
            .is_some_and(|current| generation < current)
        {
            return;
        }
        if self
            .keyed
            .get(&key)
            .is_some_and(|current| current.generation > generation)
        {
            return;
        }
        if let EngineEvent::SetPreparedSampleBank {
            instrument_slot,
            generation: bank_generation,
            ..
        } = event
        {
            let slot = usize::from(*instrument_slot);
            if self
                .sample_owner_generations
                .get(&slot)
                .is_some_and(|current| *current > *bank_generation)
            {
                return;
            }
            self.sample_owner_generations.insert(slot, *bank_generation);
        }
        if merge_mixer_event(&mut self.keyed, &key, generation, event) {
            return;
        }
        self.keyed.insert(
            key,
            ReplayValue {
                generation,
                event: event.clone(),
            },
        );
    }

    fn events(&self) -> Vec<EngineEvent> {
        let mut events = vec![self
            .audio_config
            .as_ref()
            .map(|value| value.event.clone())
            .unwrap_or_else(|| EngineEvent::SetPreparedInstruments {
                generation: 0,
                config: prepare_instruments_config(
                    default_pi_instruments(),
                    DEFAULT_AUDIO_SAMPLE_RATE,
                ),
            })];
        let mut owners = self
            .keyed
            .iter()
            .filter(|(key, _)| key.is_owner())
            .map(|(key, value)| (value.generation, *key, value.event.clone()))
            .collect::<Vec<_>>();
        owners.sort_by_key(|(generation, key, _)| (*generation, *key));
        events.extend(owners.into_iter().map(|(_, _, event)| event));

        let mut scalars = self
            .keyed
            .iter()
            .filter(|(key, value)| key.is_scalar() && self.scalar_is_current(key, value.generation))
            .map(|(key, value)| (*key, value.event.clone()))
            .collect::<Vec<_>>();
        scalars.sort_by_key(|(key, _)| *key);
        events.extend(scalars.into_iter().map(|(_, event)| event));
        events
    }

    fn scalar_is_current(&self, key: &ReplayKey, generation: u64) -> bool {
        match key.owner() {
            Some(owner) => self
                .owner_generation(owner)
                .or(self.full_generation)
                .is_some_and(|expected| generation == expected),
            None => self
                .full_generation
                .is_none_or(|expected| generation == expected),
        }
    }

    fn owner_generation(&self, owner: ReplayKey) -> Option<u64> {
        match owner {
            ReplayKey::SampleBank(slot) => self
                .sample_owner_generations
                .get(&slot)
                .copied()
                .or_else(|| self.keyed.get(&owner).map(|value| value.generation)),
            owner => self.keyed.get(&owner).map(|value| value.generation),
        }
    }

    fn preserve_atomic_sample_bank(&mut self, slot: usize) {
        if self.keyed.contains_key(&ReplayKey::SampleBank(slot)) {
            return;
        }
        let Some(previous) = self.keyed.get(&ReplayKey::InstrumentSlot(slot)).cloned() else {
            return;
        };
        let EngineEvent::SetPreparedInstrumentOwner {
            instrument_slot,
            generation,
            sample_bank: Some(bank),
            ..
        } = previous.event
        else {
            return;
        };
        self.sample_owner_generations
            .insert(slot, previous.generation);
        self.keyed.insert(
            ReplayKey::SampleBank(slot),
            ReplayValue {
                generation: previous.generation,
                event: EngineEvent::SetPreparedSampleBank {
                    instrument_slot,
                    generation,
                    bank,
                },
            },
        );
    }
}

pub(crate) fn replay_to_sink(
    tx: &EngineEventSender,
    replay_events: &Arc<Mutex<ReplayCache>>,
) -> Result<(), String> {
    let events = replay_events
        .lock()
        .map_err(|_| "audio replay cache lock failed".to_string())?;
    for event in events.events() {
        tx.send(event).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ReplayKey {
    SampleBank(usize),
    DspConfig,
    VoiceStealingMode,
    MasterVolume,
    InstrumentMixer(usize),
    InstrumentSlot(usize),
    FxBusMixer(usize),
    SynthParam(usize, SynthParamId),
    FmParam(usize, FmParamId),
    PluckParam(usize, PluckParamId),
    DrumParam(usize, usize, DrumParamId),
    SampleBankParam(usize, SampleBankParamId),
    FxBusSlot(usize, usize),
    FxBusParam(usize, usize, FxParamId),
    GlobalFxSlot(usize),
    GlobalFxParam(usize, FxParamId),
}

impl ReplayKey {
    fn is_owner(self) -> bool {
        matches!(
            self,
            Self::SampleBank(_)
                | Self::InstrumentSlot(_)
                | Self::FxBusSlot(_, _)
                | Self::GlobalFxSlot(_)
        )
    }

    fn is_scalar(self) -> bool {
        !self.is_owner()
    }

    fn owner(self) -> Option<Self> {
        match self {
            Self::InstrumentMixer(slot)
            | Self::SynthParam(slot, _)
            | Self::FmParam(slot, _)
            | Self::PluckParam(slot, _) => Some(Self::InstrumentSlot(slot)),
            Self::DrumParam(slot, _, _) => Some(Self::InstrumentSlot(slot)),
            Self::SampleBankParam(slot, _) => Some(Self::SampleBank(slot)),
            Self::FxBusParam(bus, slot, _) => Some(Self::FxBusSlot(bus, slot)),
            Self::GlobalFxParam(slot, _) => Some(Self::GlobalFxSlot(slot)),
            _ => None,
        }
    }
}

fn event_generation(event: &EngineEvent) -> u64 {
    match event {
        EngineEvent::SetPreparedSampleBank { generation, .. }
        | EngineEvent::SetPreparedInstrumentOwner { generation, .. }
        | EngineEvent::SetPreparedInstruments { generation, .. }
        | EngineEvent::SetPreparedAudioConfig { generation, .. }
        | EngineEvent::SetVoiceStealingMode { generation, .. }
        | EngineEvent::SetDspConfig { generation, .. }
        | EngineEvent::SetMasterVolume { generation, .. }
        | EngineEvent::SetInstrumentMixer { generation, .. }
        | EngineEvent::SetPreparedInstrumentSlot { generation, .. }
        | EngineEvent::SetFxBusMixer { generation, .. }
        | EngineEvent::SetSynthParam { generation, .. }
        | EngineEvent::SetFmParam { generation, .. }
        | EngineEvent::SetPluckParam { generation, .. }
        | EngineEvent::SetDrumParam { generation, .. }
        | EngineEvent::SetSampleBankParam { generation, .. }
        | EngineEvent::SetFxBusParam { generation, .. }
        | EngineEvent::SetPreparedFxBusSlot { generation, .. }
        | EngineEvent::SetGlobalFxParam { generation, .. }
        | EngineEvent::SetPreparedGlobalFxSlot { generation, .. } => *generation,
        _ => 0,
    }
}

fn replay_key(event: &EngineEvent) -> Option<ReplayKey> {
    match event {
        EngineEvent::SetPreparedSampleBank {
            instrument_slot, ..
        } => Some(ReplayKey::SampleBank(usize::from(*instrument_slot))),
        EngineEvent::SetPreparedInstrumentOwner {
            instrument_slot, ..
        } => Some(ReplayKey::InstrumentSlot(usize::from(*instrument_slot))),
        EngineEvent::SetDspConfig { .. } => Some(ReplayKey::DspConfig),
        EngineEvent::SetVoiceStealingMode { .. } => Some(ReplayKey::VoiceStealingMode),
        EngineEvent::SetMasterVolume { .. } => Some(ReplayKey::MasterVolume),
        EngineEvent::SetInstrumentMixer {
            instrument_slot, ..
        } => Some(ReplayKey::InstrumentMixer(usize::from(*instrument_slot))),
        EngineEvent::SetPreparedInstrumentSlot {
            instrument_slot, ..
        } => Some(ReplayKey::InstrumentSlot(usize::from(*instrument_slot))),
        EngineEvent::SetFxBusMixer { bus_index, .. } => {
            Some(ReplayKey::FxBusMixer(usize::from(*bus_index)))
        }
        EngineEvent::SetSynthParam {
            instrument_slot,
            param,
            ..
        } => Some(ReplayKey::SynthParam(usize::from(*instrument_slot), *param)),
        EngineEvent::SetFmParam {
            instrument_slot,
            param,
            ..
        } => Some(ReplayKey::FmParam(usize::from(*instrument_slot), *param)),
        EngineEvent::SetPluckParam {
            instrument_slot,
            param,
            ..
        } => Some(ReplayKey::PluckParam(usize::from(*instrument_slot), *param)),
        EngineEvent::SetDrumParam {
            instrument_slot,
            voice,
            param,
            ..
        } => Some(ReplayKey::DrumParam(
            usize::from(*instrument_slot),
            usize::from(*voice),
            *param,
        )),
        EngineEvent::SetSampleBankParam {
            instrument_slot,
            param,
            ..
        } => Some(ReplayKey::SampleBankParam(
            usize::from(*instrument_slot),
            *param,
        )),
        EngineEvent::SetFxBusParam {
            bus_index,
            slot_index,
            param,
            ..
        } => Some(ReplayKey::FxBusParam(
            usize::from(*bus_index),
            usize::from(*slot_index),
            *param,
        )),
        EngineEvent::SetPreparedFxBusSlot {
            bus_index,
            slot_index,
            ..
        } => Some(ReplayKey::FxBusSlot(
            usize::from(*bus_index),
            usize::from(*slot_index),
        )),
        EngineEvent::SetGlobalFxParam {
            slot_index, param, ..
        } => Some(ReplayKey::GlobalFxParam(usize::from(*slot_index), *param)),
        EngineEvent::SetPreparedGlobalFxSlot { slot_index, .. } => {
            Some(ReplayKey::GlobalFxSlot(usize::from(*slot_index)))
        }
        _ => None,
    }
}

fn merge_mixer_event(
    keyed: &mut BTreeMap<ReplayKey, ReplayValue>,
    key: &ReplayKey,
    generation: u64,
    event: &EngineEvent,
) -> bool {
    let (EngineEvent::SetFxBusMixer {
        pan_pos,
        volume_pct,
        ..
    }
    | EngineEvent::SetInstrumentMixer {
        pan_pos,
        volume_pct,
        ..
    }) = event
    else {
        return false;
    };
    let Some(value) = keyed.get_mut(key) else {
        return false;
    };
    if value.generation != generation {
        return false;
    }
    match &mut value.event {
        EngineEvent::SetFxBusMixer {
            pan_pos: queued_pan,
            volume_pct: queued_volume,
            ..
        }
        | EngineEvent::SetInstrumentMixer {
            pan_pos: queued_pan,
            volume_pct: queued_volume,
            ..
        } => {
            if pan_pos.is_some() {
                *queued_pan = *pan_pos;
            }
            if volume_pct.is_some() {
                *queued_volume = *volume_pct;
            }
            true
        }
        _ => false,
    }
}

pub(crate) fn is_replay_event(event: &EngineEvent) -> bool {
    !matches!(
        event,
        EngineEvent::AllNotesOff
            | EngineEvent::NoteOn { .. }
            | EngineEvent::DrumHit { .. }
            | EngineEvent::NoteOff { .. }
            | EngineEvent::Cc { .. }
            | EngineEvent::PreviewSample { .. }
            | EngineEvent::PreparedMomentaryFxStart { .. }
            | EngineEvent::MomentaryFxUpdate(_)
            | EngineEvent::MomentaryFxStop { .. }
            | EngineEvent::ProbeMark { .. }
    )
}

#[cfg(test)]
pub(crate) fn collect_replay_events(cache: &ReplayCache) -> Vec<EngineEvent> {
    cache.events()
}

#[cfg(test)]
#[path = "audio_replay_drum_tests.rs"]
mod drum_tests;
#[cfg(test)]
#[path = "audio_replay_tests.rs"]
mod tests;
