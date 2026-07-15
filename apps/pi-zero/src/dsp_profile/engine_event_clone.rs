pub(super) fn clone_event(
    event: &rodio_engine_source::EngineEvent,
) -> rodio_engine_source::EngineEvent {
    use rodio_engine_source::EngineEvent;

    match event {
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
        EngineEvent::SetInstruments(config) => EngineEvent::SetInstruments(config.clone()),
        EngineEvent::SetSampleBanks(banks) => EngineEvent::SetSampleBanks(banks.clone()),
        EngineEvent::SetSampleBank {
            instrument_slot,
            bank,
        } => EngineEvent::SetSampleBank {
            instrument_slot: *instrument_slot,
            bank: bank.clone(),
        },
        EngineEvent::SetAudioConfig {
            instruments,
            sample_banks,
            voice_stealing_mode,
        } => EngineEvent::SetAudioConfig {
            instruments: instruments.clone(),
            sample_banks: sample_banks.clone(),
            voice_stealing_mode: *voice_stealing_mode,
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
        EngineEvent::SetInstrumentSlot {
            instrument_slot,
            config,
        } => EngineEvent::SetInstrumentSlot {
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
        EngineEvent::SetFxBusSlot {
            bus_index,
            slot_index,
            fx_type,
            params,
        } => EngineEvent::SetFxBusSlot {
            bus_index: *bus_index,
            slot_index: *slot_index,
            fx_type: fx_type.clone(),
            params: params.clone(),
        },
        EngineEvent::SetGlobalFxSlot {
            slot_index,
            fx_type,
            params,
        } => EngineEvent::SetGlobalFxSlot {
            slot_index: *slot_index,
            fx_type: fx_type.clone(),
            params: params.clone(),
        },
        EngineEvent::MomentaryFxStart {
            id,
            fx_type,
            params,
            target,
        } => EngineEvent::MomentaryFxStart {
            id: id.clone(),
            fx_type: fx_type.clone(),
            params: params.clone(),
            target: *target,
        },
        EngineEvent::MomentaryFxUpdate { id, params } => EngineEvent::MomentaryFxUpdate {
            id: id.clone(),
            params: params.clone(),
        },
        EngineEvent::MomentaryFxStop { id } => EngineEvent::MomentaryFxStop { id: id.clone() },
        EngineEvent::ProbeMark { .. } => panic!("probe marks cannot be cloned for DSP profiling"),
    }
}
