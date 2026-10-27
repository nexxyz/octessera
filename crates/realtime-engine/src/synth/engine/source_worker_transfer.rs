use super::super::types::INSTRUMENT_SLOT_COUNT;
use super::source_worker_lease::OwnerLease;
use super::source_worker_lifecycle::SourceLanePartitionBundle;
use super::SynthEngine;

pub(super) fn take_source_partition_bundle(
    engine: &mut SynthEngine,
    parity: usize,
) -> Option<SourceLanePartitionBundle> {
    if parity >= 2
        || !engine.synth_voice_pool.partition_is_present(parity)
        || !engine.sample_voice_pool.partition_is_present(parity)
    {
        return None;
    }
    let synth = engine.synth_voice_pool.take_partition(parity)?;
    let Some(sample) = engine.sample_voice_pool.take_partition(parity) else {
        engine
            .synth_voice_pool
            .install_partition_after_vacancy_check(parity, synth);
        return None;
    };
    Some(SourceLanePartitionBundle { synth, sample })
}

pub(super) fn can_install_source_partition_bundle(engine: &SynthEngine, parity: usize) -> bool {
    parity < 2
        && engine.synth_voice_pool.partition_is_vacant(parity)
        && engine.sample_voice_pool.partition_is_vacant(parity)
}

pub(super) fn source_partitions_vacant(engine: &SynthEngine) -> bool {
    (0..2).all(|parity| {
        engine.synth_voice_pool.partition_is_vacant(parity)
            && engine.sample_voice_pool.partition_is_vacant(parity)
    })
}

pub(super) fn with_both_source_partitions_read_only<R>(
    engine: &mut SynthEngine,
    first: &mut OwnerLease,
    second: &mut OwnerLease,
    inspect: impl FnOnce(&SynthEngine) -> R,
) -> Result<R, ()> {
    super::source_worker_carrier_transfer::with_both_source_owners(
        engine,
        first,
        second,
        |engine, _| inspect(engine),
    )
}

pub(super) fn install_source_partition_bundle_after_check(
    engine: &mut SynthEngine,
    parity: usize,
    partitions: SourceLanePartitionBundle,
) {
    let SourceLanePartitionBundle { synth, sample } = partitions;
    engine
        .sample_voice_pool
        .install_partition_after_vacancy_check(parity, sample);
    engine
        .synth_voice_pool
        .install_partition_after_vacancy_check(parity, synth);
}

pub(super) fn take_source_partition_bundles(
    engine: &mut SynthEngine,
) -> Option<(SourceLanePartitionBundle, SourceLanePartitionBundle)> {
    let first = take_source_partition_bundle(engine, 0)?;
    let Some(second) = take_source_partition_bundle(engine, 1) else {
        restore_source_partition_bundle(engine, first);
        return None;
    };
    Some((first, second))
}

pub(super) fn restore_source_partition_bundle(
    engine: &mut SynthEngine,
    partitions: SourceLanePartitionBundle,
) {
    let SourceLanePartitionBundle { synth, sample } = partitions;
    let parity = synth.parity();
    engine
        .sample_voice_pool
        .install_partition_after_vacancy_check(parity, sample);
    engine
        .synth_voice_pool
        .install_partition_after_vacancy_check(parity, synth);
}

pub(super) fn compact_source_pools(engine: &mut SynthEngine) {
    for slot in 0..INSTRUMENT_SLOT_COUNT {
        engine.synth_voice_pool.compact_slot_lanes(slot);
        engine.sample_voice_pool.compact_slot_lanes(slot);
        engine.active_synth_slots[slot] = false;
        engine.active_sample_slots[slot] = false;
    }
}
