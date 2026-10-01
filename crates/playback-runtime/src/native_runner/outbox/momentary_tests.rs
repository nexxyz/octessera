use super::*;
use std::collections::BTreeMap;

fn start(id: &str, epoch: u64) -> RuntimeAudioCommand {
    RuntimeAudioCommand::MomentaryFxStart {
        id: id.into(),
        epoch,
        fx_type: "stutter".into(),
        params: BTreeMap::new(),
        target: crate::protocol::RuntimeMomentaryFxTarget::Global,
    }
}

fn update(id: &str, epoch: u64) -> RuntimeAudioCommand {
    RuntimeAudioCommand::MomentaryFxUpdate {
        id: id.into(),
        epoch,
        params: BTreeMap::new(),
    }
}

fn stop(id: &str, epoch: u64) -> RuntimeAudioCommand {
    RuntimeAudioCommand::MomentaryFxStop {
        id: id.into(),
        epoch,
    }
}

#[test]
fn different_ids_get_distinct_epochs_and_keep_updates_and_stops_isolated() {
    let mut outbox = NativeRunnerOutbox::default();
    outbox.push_audio_command(start("a", 0));
    outbox.push_audio_command(start("b", 0));
    outbox.push_audio_command(update("a", 0));
    outbox.push_audio_command(update("b", 0));
    outbox.push_audio_command(stop("a", 0));
    outbox.push_audio_command(stop("b", 0));
    let commands = outbox.drain_audio_commands();

    assert!(matches!(
        commands[0],
        RuntimeAudioCommand::MomentaryFxStart { ref id, epoch: 1, .. } if id == "a"
    ));
    assert!(matches!(
        commands[1],
        RuntimeAudioCommand::MomentaryFxStart { ref id, epoch: 2, .. } if id == "b"
    ));
    assert!(matches!(
        commands[2],
        RuntimeAudioCommand::MomentaryFxUpdate { ref id, epoch: 1, .. } if id == "a"
    ));
    assert!(matches!(
        commands[3],
        RuntimeAudioCommand::MomentaryFxUpdate { ref id, epoch: 2, .. } if id == "b"
    ));
    assert!(matches!(
        commands[4],
        RuntimeAudioCommand::MomentaryFxStop { ref id, epoch: 1 } if id == "a"
    ));
    assert!(matches!(
        commands[5],
        RuntimeAudioCommand::MomentaryFxStop { ref id, epoch: 2 } if id == "b"
    ));
}

#[test]
fn platform_effects_share_the_same_global_epoch_identity() {
    let mut outbox = NativeRunnerOutbox::default();
    for command in [start("a", 0), start("b", 0), update("a", 0)] {
        outbox.push_platform_effect(RuntimePlatformEffect::AudioCommand { command });
    }
    let effects = outbox.drain_platform_effects();

    assert!(matches!(
        effects[0],
        RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::MomentaryFxStart { ref id, epoch: 1, .. }
        } if id == "a"
    ));
    assert!(matches!(
        effects[1],
        RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::MomentaryFxStart { ref id, epoch: 2, .. }
        } if id == "b"
    ));
    assert!(matches!(
        effects[2],
        RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::MomentaryFxUpdate { ref id, epoch: 1, .. }
        } if id == "a"
    ));
}

#[test]
fn same_id_restart_gets_a_new_epoch_and_stale_stop_cannot_clear_current() {
    let mut outbox = NativeRunnerOutbox::default();
    outbox.push_audio_command(start("a", 0));
    outbox.push_audio_command(start("a", 0));
    outbox.push_audio_command(update("a", 0));
    outbox.push_audio_command(stop("a", 1));
    outbox.push_audio_command(update("a", 0));
    outbox.push_audio_command(stop("a", 0));
    let commands = outbox.drain_audio_commands();

    assert_eq!(commands.len(), 5);
    assert!(matches!(
        commands[0],
        RuntimeAudioCommand::MomentaryFxStart { epoch: 1, .. }
    ));
    assert!(matches!(
        commands[1],
        RuntimeAudioCommand::MomentaryFxStart { epoch: 2, .. }
    ));
    assert!(matches!(
        commands[2],
        RuntimeAudioCommand::MomentaryFxUpdate { epoch: 2, .. }
    ));
    assert!(matches!(
        commands[3],
        RuntimeAudioCommand::MomentaryFxStop { epoch: 1, .. }
    ));
    assert!(matches!(
        commands[4],
        RuntimeAudioCommand::MomentaryFxStop { epoch: 2, .. }
    ));
}

#[test]
fn explicit_duplicate_or_stale_starts_rebase_to_fresh_epochs() {
    let mut outbox = NativeRunnerOutbox::default();
    outbox.push_audio_command(start("a", 9));
    outbox.push_audio_command(start("b", 9));
    outbox.push_audio_command(stop("a", 9));
    outbox.push_audio_command(start("c", 9));
    let commands = outbox.drain_audio_commands();

    assert!(matches!(
        commands[0],
        RuntimeAudioCommand::MomentaryFxStart { epoch: 9, .. }
    ));
    assert!(matches!(
        commands[1],
        RuntimeAudioCommand::MomentaryFxStart { epoch: 10, .. }
    ));
    assert!(matches!(
        commands[2],
        RuntimeAudioCommand::MomentaryFxStop { epoch: 9, .. }
    ));
    assert!(matches!(
        commands[3],
        RuntimeAudioCommand::MomentaryFxStart { epoch: 11, .. }
    ));
}

#[test]
fn explicit_stale_update_is_not_rewritten_to_the_current_epoch() {
    let mut outbox = NativeRunnerOutbox::default();
    outbox.push_audio_command(start("a", 0));
    outbox.push_audio_command(start("a", 0));
    let stale = outbox.stamp_platform_effect(RuntimePlatformEffect::AudioCommand {
        command: update("a", 1),
    });

    assert!(matches!(
        stale,
        RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::MomentaryFxUpdate { epoch: 1, .. }
        }
    ));
}

#[test]
fn two_active_effects_and_wrap_avoid_reserved_epochs_and_collisions() {
    let mut outbox = NativeRunnerOutbox {
        momentary_epoch: u64::MAX - 1,
        ..Default::default()
    };
    outbox.push_audio_command(start("a", u64::MAX));
    outbox.push_audio_command(start("b", 0));
    let commands = outbox.drain_audio_commands();

    assert!(matches!(
        commands[0],
        RuntimeAudioCommand::MomentaryFxStart { epoch: 1, .. }
    ));
    assert!(matches!(
        commands[1],
        RuntimeAudioCommand::MomentaryFxStart { epoch: 2, .. }
    ));

    let mut collision_outbox = NativeRunnerOutbox {
        momentary_epoch: u64::MAX,
        momentary_epoch_wrapped: true,
        active_momentary_epochs: BTreeMap::from([(String::from("b"), 1)]),
        ..Default::default()
    };
    collision_outbox.push_audio_command(start("c", 0));
    let commands = collision_outbox.drain_audio_commands();
    assert!(matches!(
        commands[0],
        RuntimeAudioCommand::MomentaryFxStart { epoch: 2, .. }
    ));

    collision_outbox.push_audio_command(stop("b", 1));
    let _ = collision_outbox.drain_audio_commands();
    collision_outbox.push_audio_command(start("d", u64::MAX));
    let commands = collision_outbox.drain_audio_commands();
    assert!(matches!(
        commands[0],
        RuntimeAudioCommand::MomentaryFxStart { epoch: 3, .. }
    ));
}
