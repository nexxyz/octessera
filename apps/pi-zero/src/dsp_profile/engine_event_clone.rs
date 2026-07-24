pub(super) fn clone_event(
    event: &rodio_engine_source::EngineEvent,
) -> rodio_engine_source::EngineEvent {
    use rodio_engine_source::EngineEvent;

    match event {
        EngineEvent::AllNotesOff => EngineEvent::AllNotesOff,
        EngineEvent::NoteOn {
            instrument_slot,
            note,
            velocity,
            duration_ms,
        } => EngineEvent::NoteOn {
            instrument_slot: *instrument_slot,
            note: *note,
            velocity: *velocity,
            duration_ms: *duration_ms,
        },
        EngineEvent::NoteOff {
            instrument_slot,
            note,
        } => EngineEvent::NoteOff {
            instrument_slot: *instrument_slot,
            note: *note,
        },
        EngineEvent::Cc {
            instrument_slot,
            controller,
            value,
        } => EngineEvent::Cc {
            instrument_slot: *instrument_slot,
            controller: *controller,
            value: *value,
        },
        EngineEvent::SetPreparedInstruments(config) => {
            EngineEvent::SetPreparedInstruments(config.clone())
        }
        EngineEvent::SetPreparedAudioConfig(config) => {
            EngineEvent::SetPreparedAudioConfig(config.clone())
        }
        EngineEvent::SetPreparedSampleBank {
            instrument_slot,
            bank,
        } => EngineEvent::SetPreparedSampleBank {
            instrument_slot: *instrument_slot,
            bank: bank.clone(),
        },
        EngineEvent::PreviewSample {
            instrument_slot,
            buffer,
            velocity,
        } => EngineEvent::PreviewSample {
            instrument_slot: *instrument_slot,
            buffer: buffer.clone(),
            velocity: *velocity,
        },
        EngineEvent::SetVoiceStealingMode(mode) => EngineEvent::SetVoiceStealingMode(*mode),
        EngineEvent::SetMasterVolume { volume_pct } => EngineEvent::SetMasterVolume {
            volume_pct: *volume_pct,
        },
        EngineEvent::SetInstrumentMixer {
            instrument_slot,
            volume_pct,
            pan_pos,
        } => EngineEvent::SetInstrumentMixer {
            instrument_slot: *instrument_slot,
            volume_pct: *volume_pct,
            pan_pos: *pan_pos,
        },
        EngineEvent::SetPreparedInstrumentSlot {
            instrument_slot,
            config,
        } => EngineEvent::SetPreparedInstrumentSlot {
            instrument_slot: *instrument_slot,
            config: config.clone(),
        },
        EngineEvent::SetFxBusMixer {
            bus_index,
            pan_pos,
            volume_pct,
        } => EngineEvent::SetFxBusMixer {
            bus_index: *bus_index,
            pan_pos: *pan_pos,
            volume_pct: *volume_pct,
        },
        EngineEvent::SetSynthParam {
            instrument_slot,
            path,
            value,
        } => EngineEvent::SetSynthParam {
            instrument_slot: *instrument_slot,
            path: path.clone(),
            value: *value,
        },
        EngineEvent::SetSampleBankParam {
            instrument_slot,
            path,
            value,
        } => EngineEvent::SetSampleBankParam {
            instrument_slot: *instrument_slot,
            path: path.clone(),
            value: *value,
        },
        EngineEvent::SetPreparedFxBusSlot {
            bus_index,
            slot_index,
            config,
        } => EngineEvent::SetPreparedFxBusSlot {
            bus_index: *bus_index,
            slot_index: *slot_index,
            config: config.clone(),
        },
        EngineEvent::SetPreparedGlobalFxSlot { slot_index, config } => {
            EngineEvent::SetPreparedGlobalFxSlot {
                slot_index: *slot_index,
                config: config.clone(),
            }
        }
        EngineEvent::PreparedMomentaryFxStart(config) => {
            EngineEvent::PreparedMomentaryFxStart(config.clone())
        }
        EngineEvent::MomentaryFxUpdate { id, params } => EngineEvent::MomentaryFxUpdate {
            id: id.clone(),
            params: params.clone(),
        },
        EngineEvent::MomentaryFxStop { id } => EngineEvent::MomentaryFxStop { id: id.clone() },
        EngineEvent::ProbeMark { .. } => panic!("probe marks cannot be cloned for DSP profiling"),
    }
}
