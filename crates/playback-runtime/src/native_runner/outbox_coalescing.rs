use super::super::instrument_payload_owns_sample_bank;
use crate::protocol::RuntimeAudioCommand;
use crate::protocol::RuntimePlatformEffect;
use std::collections::BTreeMap;

pub(super) fn push_audio_command(
    commands: &mut Vec<RuntimeAudioCommand>,
    command: RuntimeAudioCommand,
    active_momentary_epochs: &BTreeMap<String, u64>,
) {
    if matches!(command, RuntimeAudioCommand::SetAudioConfig { .. }) {
        commands.retain(is_transient_command);
        commands.insert(0, command);
        return;
    }
    if let RuntimeAudioCommand::MomentaryFxUpdate { ref id, epoch, .. } = command {
        if active_momentary_epochs.get(id) != Some(&epoch) {
            return;
        }
        let start = matching_momentary_start(commands, id, epoch);
        let Some(start) = start else {
            if let Some(index) = commands.iter().position(|queued| {
                matches!(
                    queued,
                    RuntimeAudioCommand::MomentaryFxUpdate {
                        id: queued_id,
                        epoch: queued_epoch,
                        ..
                    } if queued_id == id && *queued_epoch == epoch
                )
            }) {
                commands[index] = command;
            } else {
                commands.push(command);
            }
            return;
        };
        if let Some(index) =
            commands
                .iter()
                .enumerate()
                .skip(start + 1)
                .find_map(|(index, queued)| {
                    matches!(
                        queued,
                        RuntimeAudioCommand::MomentaryFxUpdate {
                            id: queued_id,
                            epoch: queued_epoch,
                            ..
                        } if queued_id == id && *queued_epoch == epoch
                    )
                    .then_some(index)
                })
        {
            commands[index] = command;
        } else {
            commands.push(command);
        }
        return;
    }
    if let Some(index) = scalar_command_index(commands, &command) {
        commands[index] = command;
        return;
    }
    if let Some(merged) = merge_mixer_command(commands, command) {
        commands.push(merged);
    }
}

pub(super) fn push_platform_audio_command(
    effects: &mut Vec<RuntimePlatformEffect>,
    command: RuntimeAudioCommand,
    active_momentary_epochs: &BTreeMap<String, u64>,
) {
    if let RuntimeAudioCommand::MomentaryFxUpdate { ref id, epoch, .. } = command {
        if active_momentary_epochs.get(id) != Some(&epoch) {
            return;
        }
        if let Some(index) = effects.iter().position(|effect| {
            matches!(
                effect,
                RuntimePlatformEffect::AudioCommand {
                    command: RuntimeAudioCommand::MomentaryFxUpdate {
                        id: queued_id,
                        epoch: queued_epoch,
                        ..
                    }
                } if queued_id == id && *queued_epoch == epoch
            )
        }) {
            effects[index] = RuntimePlatformEffect::AudioCommand { command };
        } else {
            effects.push(RuntimePlatformEffect::AudioCommand { command });
        }
        return;
    }
    effects.push(RuntimePlatformEffect::AudioCommand { command });
}

fn is_transient_command(command: &RuntimeAudioCommand) -> bool {
    matches!(
        command,
        RuntimeAudioCommand::MomentaryFxStart { .. }
            | RuntimeAudioCommand::MomentaryFxUpdate { .. }
            | RuntimeAudioCommand::MomentaryFxStop { .. }
            | RuntimeAudioCommand::SamplePreview { .. }
    )
}

fn scalar_command_index(
    commands: &mut Vec<RuntimeAudioCommand>,
    command: &RuntimeAudioCommand,
) -> Option<usize> {
    if is_replacement(command) {
        commands.retain(|queued| !same_replacement_owner(queued, command));
        return None;
    }
    commands
        .iter()
        .position(|queued| same_scalar_key(queued, command))
}

fn is_replacement(command: &RuntimeAudioCommand) -> bool {
    matches!(
        command,
        RuntimeAudioCommand::SetInstrumentSlot { .. }
            | RuntimeAudioCommand::SetFxBusSlot { .. }
            | RuntimeAudioCommand::SetGlobalFxSlot { .. }
    )
}

fn same_replacement_owner(left: &RuntimeAudioCommand, right: &RuntimeAudioCommand) -> bool {
    match right {
        RuntimeAudioCommand::SetInstrumentSlot {
            instrument_slot: right_slot,
            config: right_config,
            ..
        } => match left {
            RuntimeAudioCommand::SetInstrumentSlot {
                instrument_slot: left_slot,
                ..
            }
            | RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: left_slot,
                ..
            }
            | RuntimeAudioCommand::SetSynthParam {
                instrument_slot: left_slot,
                ..
            }
            | RuntimeAudioCommand::SetSampleBankParam {
                instrument_slot: left_slot,
                ..
            } => {
                left_slot == right_slot
                    && (!matches!(left, RuntimeAudioCommand::SetSampleBankParam { .. })
                        || instrument_payload_owns_sample_bank(right_config))
            }
            _ => false,
        },
        RuntimeAudioCommand::SetFxBusSlot {
            bus_index: right_bus,
            slot_index: right_slot,
            ..
        } => matches!(
            left,
            RuntimeAudioCommand::SetFxBusSlot {
                bus_index: left_bus,
                slot_index: left_slot,
                ..
            }
            | RuntimeAudioCommand::SetFxBusParam {
                bus_index: left_bus,
                slot_index: left_slot,
                ..
            } if left_bus == right_bus && left_slot == right_slot
        ),
        RuntimeAudioCommand::SetGlobalFxSlot {
            slot_index: right_slot,
            ..
        } => matches!(
            left,
            RuntimeAudioCommand::SetGlobalFxSlot {
                slot_index: left_slot,
                ..
            }
            | RuntimeAudioCommand::SetGlobalFxParam {
                slot_index: left_slot,
                ..
            } if left_slot == right_slot
        ),
        _ => false,
    }
}

fn same_scalar_key(left: &RuntimeAudioCommand, right: &RuntimeAudioCommand) -> bool {
    match (left, right) {
        (RuntimeAudioCommand::SetDspConfig { .. }, RuntimeAudioCommand::SetDspConfig { .. })
        | (
            RuntimeAudioCommand::SetMasterVolume { .. },
            RuntimeAudioCommand::SetMasterVolume { .. },
        ) => true,
        (
            RuntimeAudioCommand::SetSynthParam {
                instrument_slot: left_slot,
                path: left_path,
                ..
            },
            RuntimeAudioCommand::SetSynthParam {
                instrument_slot: right_slot,
                path: right_path,
                ..
            },
        ) => left_slot == right_slot && left_path == right_path,
        (
            RuntimeAudioCommand::SetSampleBankParam {
                instrument_slot: left_slot,
                path: left_path,
                ..
            },
            RuntimeAudioCommand::SetSampleBankParam {
                instrument_slot: right_slot,
                path: right_path,
                ..
            },
        ) => left_slot == right_slot && left_path == right_path,
        (
            RuntimeAudioCommand::SetFxBusParam {
                bus_index: left_bus,
                slot_index: left_slot,
                param: left_param,
                ..
            },
            RuntimeAudioCommand::SetFxBusParam {
                bus_index: right_bus,
                slot_index: right_slot,
                param: right_param,
                ..
            },
        ) => left_bus == right_bus && left_slot == right_slot && left_param == right_param,
        (
            RuntimeAudioCommand::SetGlobalFxParam {
                slot_index: left_slot,
                param: left_param,
                ..
            },
            RuntimeAudioCommand::SetGlobalFxParam {
                slot_index: right_slot,
                param: right_param,
                ..
            },
        ) => left_slot == right_slot && left_param == right_param,
        _ => false,
    }
}

fn merge_mixer_command(
    commands: &mut [RuntimeAudioCommand],
    command: RuntimeAudioCommand,
) -> Option<RuntimeAudioCommand> {
    match command {
        RuntimeAudioCommand::SetInstrumentMixer {
            instrument_slot,
            generation,
            volume_pct,
            pan_pos,
        } => {
            if let Some(RuntimeAudioCommand::SetInstrumentMixer {
                generation: queued_generation,
                volume_pct: queued_volume,
                pan_pos: queued_pan,
                ..
            }) = commands.iter_mut().find(|queued| {
                matches!(
                    queued,
                    RuntimeAudioCommand::SetInstrumentMixer {
                        instrument_slot: queued_slot,
                        ..
                    } if *queued_slot == instrument_slot
                )
            }) {
                *queued_generation = generation;
                if volume_pct.is_some() {
                    *queued_volume = volume_pct;
                }
                if pan_pos.is_some() {
                    *queued_pan = pan_pos;
                }
                return None;
            }
            Some(RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot,
                generation,
                volume_pct,
                pan_pos,
            })
        }
        RuntimeAudioCommand::SetFxBusMixer {
            bus_index,
            generation,
            pan_pos,
            volume_pct,
        } => {
            if let Some(RuntimeAudioCommand::SetFxBusMixer {
                generation: queued_generation,
                pan_pos: queued_pan,
                volume_pct: queued_volume,
                ..
            }) = commands.iter_mut().find(|queued| {
                matches!(
                    queued,
                    RuntimeAudioCommand::SetFxBusMixer {
                        bus_index: queued_bus,
                        ..
                    } if *queued_bus == bus_index
                )
            }) {
                *queued_generation = generation;
                if pan_pos.is_some() {
                    *queued_pan = pan_pos;
                }
                if volume_pct.is_some() {
                    *queued_volume = volume_pct;
                }
                return None;
            }
            Some(RuntimeAudioCommand::SetFxBusMixer {
                bus_index,
                generation,
                pan_pos,
                volume_pct,
            })
        }
        command => Some(command),
    }
}

fn matching_momentary_start(
    commands: &[RuntimeAudioCommand],
    id: &str,
    epoch: u64,
) -> Option<usize> {
    let mut start = None;
    for (index, command) in commands.iter().enumerate() {
        match command {
            RuntimeAudioCommand::MomentaryFxStart {
                id: command_id,
                epoch: command_epoch,
                ..
            } if command_id == id && *command_epoch == epoch => start = Some(index),
            RuntimeAudioCommand::MomentaryFxStop {
                id: command_id,
                epoch: command_epoch,
            } if command_id == id && *command_epoch == epoch => start = None,
            _ => {}
        }
    }
    start
}
