use super::{apply_runtime_audio_command, MomentaryFxTypes, RuntimeAudioCommandHost};
use crate::{
    RuntimeAdapterError, RuntimeAudioCommand, RuntimeErrorCode, RuntimeErrorDomain,
    RuntimeErrorFacts, RuntimeMomentaryFxTarget, RuntimeOperation,
};
use realtime_engine::synth::{DrumParamId, EngineEvent};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Default)]
struct RecordingHost {
    events: Vec<EngineEvent>,
    prepared: Vec<String>,
    momentary: MomentaryFxTypes,
    engine_gone: bool,
}

impl RuntimeAudioCommandHost for RecordingHost {
    fn send_audio_event(&mut self, event: EngineEvent) -> Result<(), RuntimeAdapterError> {
        if self.engine_gone {
            return Err(RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
                RuntimeErrorDomain::Audio,
                RuntimeErrorCode::AudioThreadFailed,
                RuntimeOperation::AudioCommand,
                None,
            )));
        }
        self.events.push(event);
        Ok(())
    }

    fn enqueue_full_config(
        &mut self,
        revision: u64,
        _: u64,
        _: Option<String>,
        _: Value,
    ) -> Result<(), RuntimeAdapterError> {
        self.prepared.push(format!("full {revision}"));
        Ok(())
    }

    fn enqueue_instrument_slot(
        &mut self,
        instrument_slot: usize,
        _: u64,
        _: Value,
    ) -> Result<(), RuntimeAdapterError> {
        self.prepared.push(format!("slot {instrument_slot}"));
        Ok(())
    }

    fn enqueue_fx_bus_slot(
        &mut self,
        bus_index: usize,
        slot_index: usize,
        _: u64,
        fx_type: String,
        _: BTreeMap<String, Value>,
    ) -> Result<(), RuntimeAdapterError> {
        self.prepared
            .push(format!("bus {bus_index}.{slot_index} {fx_type}"));
        Ok(())
    }

    fn enqueue_global_fx_slot(
        &mut self,
        slot_index: usize,
        _: u64,
        fx_type: String,
        _: BTreeMap<String, Value>,
    ) -> Result<(), RuntimeAdapterError> {
        self.prepared.push(format!("global {slot_index} {fx_type}"));
        Ok(())
    }

    fn enqueue_sample_preview(
        &mut self,
        instrument_slot: usize,
        path: String,
        velocity: u8,
    ) -> Result<(), RuntimeAdapterError> {
        self.prepared
            .push(format!("preview {instrument_slot} {path} {velocity}"));
        Ok(())
    }

    fn momentary_fx_types(&mut self) -> &mut MomentaryFxTypes {
        &mut self.momentary
    }
}

fn start(id: &str, epoch: u64) -> RuntimeAudioCommand {
    RuntimeAudioCommand::MomentaryFxStart {
        id: id.into(),
        epoch,
        fx_type: "freeze".into(),
        params: BTreeMap::new(),
        target: RuntimeMomentaryFxTarget::Global,
    }
}

fn invalid_message(host: &mut RecordingHost, command: RuntimeAudioCommand) -> String {
    let error = apply_runtime_audio_command(host, &command).unwrap_err();
    assert_eq!(error.facts.code, RuntimeErrorCode::InvalidPayload);
    assert_eq!(error.facts.operation, RuntimeOperation::AudioCommand);
    error.facts.message.unwrap_or_default()
}

#[test]
fn string_paths_reach_the_engine_as_native_parameter_ids() {
    let mut host = RecordingHost::default();

    apply_runtime_audio_command(
        &mut host,
        &RuntimeAudioCommand::SetDrumParam {
            instrument_slot: 3,
            voice: 2,
            generation: 9,
            path: "drum.tuneSemis".into(),
            value: 0.25,
        },
    )
    .unwrap();

    let [EngineEvent::SetDrumParam {
        instrument_slot: 3,
        voice: 2,
        generation: 9,
        param,
        value,
    }] = host.events.as_slice()
    else {
        panic!(
            "expected one Drum parameter event, got {}",
            host.events.len()
        );
    };
    assert_eq!(*param, DrumParamId::from_path("drum.tuneSemis").unwrap());
    assert_eq!(*value, 0.25);
}

#[test]
fn structural_commands_and_previews_go_to_the_preparation_lane() {
    let mut host = RecordingHost::default();
    let commands = [
        RuntimeAudioCommand::SetInstrumentSlot {
            instrument_slot: 1,
            generation: 0,
            config: json!({ "type": "synth" }),
        },
        RuntimeAudioCommand::SetFxBusSlot {
            bus_index: 1,
            slot_index: 2,
            generation: 0,
            fx_type: "delay".into(),
            params: BTreeMap::new(),
        },
        RuntimeAudioCommand::SetGlobalFxSlot {
            slot_index: 0,
            generation: 0,
            fx_type: "reverb".into(),
            params: BTreeMap::new(),
        },
        RuntimeAudioCommand::SamplePreview {
            instrument_slot: 4,
            path: "kick.wav".into(),
            sample_slot: 0,
            velocity: 100,
        },
    ];

    for command in &commands {
        apply_runtime_audio_command(&mut host, command).unwrap();
    }

    assert!(host.events.is_empty());
    assert_eq!(
        host.prepared,
        [
            "slot 1",
            "bus 1.2 delay",
            "global 0 reverb",
            "preview 4 kick.wav 100"
        ]
    );
}

#[test]
fn momentary_updates_and_stops_need_the_active_epoch_and_name_the_fault() {
    let mut host = RecordingHost::default();
    apply_runtime_audio_command(&mut host, &start("fx", 2)).unwrap();

    let stale_update = invalid_message(
        &mut host,
        RuntimeAudioCommand::MomentaryFxUpdate {
            id: "fx".into(),
            epoch: 1,
            params: BTreeMap::new(),
        },
    );
    let unknown_stop = invalid_message(
        &mut host,
        RuntimeAudioCommand::MomentaryFxStop {
            id: "other".into(),
            epoch: 2,
        },
    );

    assert_eq!(stale_update, "stale momentary FX epoch for `fx`");
    assert_eq!(unknown_stop, "unknown momentary FX id `other`");
    assert_eq!(host.events.len(), 1);
    assert_eq!(host.momentary.get("fx"), Some(&(2, "freeze".to_string())));

    apply_runtime_audio_command(
        &mut host,
        &RuntimeAudioCommand::MomentaryFxStop {
            id: "fx".into(),
            epoch: 2,
        },
    )
    .unwrap();
    assert!(matches!(
        host.events.last(),
        Some(EngineEvent::MomentaryFxStop { epoch: 2 })
    ));
    assert!(host.momentary.is_empty());
}

#[test]
fn a_momentary_start_or_stop_the_engine_did_not_take_leaves_the_state_alone() {
    let mut host = RecordingHost::default();
    apply_runtime_audio_command(&mut host, &start("fx", 3)).unwrap();
    host.engine_gone = true;

    let stop = apply_runtime_audio_command(
        &mut host,
        &RuntimeAudioCommand::MomentaryFxStop {
            id: "fx".into(),
            epoch: 3,
        },
    );
    let restart = apply_runtime_audio_command(&mut host, &start("other", 4));

    assert_eq!(
        stop.unwrap_err().facts.code,
        RuntimeErrorCode::AudioThreadFailed
    );
    assert!(restart.is_err());
    assert_eq!(host.momentary.len(), 1);
    assert_eq!(host.momentary.get("fx"), Some(&(3, "freeze".to_string())));
}

#[test]
fn an_unknown_momentary_fx_type_is_refused_before_the_engine() {
    let mut host = RecordingHost::default();
    let error = apply_runtime_audio_command(
        &mut host,
        &RuntimeAudioCommand::MomentaryFxStart {
            id: "fx".into(),
            epoch: 1,
            fx_type: "unknown".into(),
            params: BTreeMap::new(),
            target: RuntimeMomentaryFxTarget::Global,
        },
    )
    .unwrap_err();

    assert_eq!(error.facts.code, RuntimeErrorCode::InvalidPayload);
    assert!(host.events.is_empty());
    assert!(host.momentary.is_empty());
}
