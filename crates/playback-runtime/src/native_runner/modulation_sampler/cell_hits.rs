use super::super::drum_config::drum_velocity_level;
use super::super::sample_assignment_payload::cell_offset;
use super::super::{NativeInstrumentSlot, NativeSampleAssignment, GRID_WIDTH};
use crate::protocol::CellHit;
use platform_core::CellTriggerIntent;
use realtime_engine::synth::CELL_PITCH_CENTS_LIMIT;
use serde_json::Value;

pub(in super::super) fn cell_index(x: usize, y: usize) -> u8 {
    (y * GRID_WIDTH + x) as u8
}

pub(in super::super) fn drum_hit_for_intent(
    instruments: &[NativeInstrumentSlot],
    slot: u8,
    intent: &CellTriggerIntent,
    velocity: u8,
    transpose_offset: i8,
    link_note: Option<u8>,
) -> Option<CellHit> {
    let instrument = instruments.get(slot as usize)?;
    if instrument.kind != "drum" {
        return None;
    }
    let assignment = instrument
        .drum_config
        .get("assignments")?
        .as_array()?
        .iter()
        .find(|cell| {
            cell.get("x").and_then(Value::as_u64) == Some(intent.x as u64)
                && cell.get("y").and_then(Value::as_u64) == Some(intent.y as u64)
        })?;
    let voice = u8::try_from(assignment.get("voice")?.as_u64()?).ok()?;
    if voice > 7 {
        return None;
    }
    let tune_semis = cell_offset(assignment, "tuneSemis", 24);
    let tune_cents = cell_offset(assignment, "tuneCents", 100);
    let level = assignment.get("level").and_then(Value::as_str);
    let flag = |key: &str| {
        instrument
            .drum_config
            .get(key)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    Some(CellHit {
        instrument_slot: slot,
        voice,
        cell: cell_index(intent.x, intent.y),
        pitch_cents: cell_pitch_cents(
            tune_semis,
            tune_cents,
            flag("followTranspose").then_some(transpose_offset),
            link_note.filter(|_| flag("followPitch")),
        ),
        velocity: match level.and_then(|level| drum_velocity_level(&instrument.drum_config, level))
        {
            Some(level_velocity) => scale_velocity(level_velocity, velocity),
            None => velocity,
        },
    })
}

pub(in super::super) fn sampler_hit_for_intent(
    instrument: &NativeInstrumentSlot,
    slot: u8,
    intent: &CellTriggerIntent,
    velocity: u8,
    transpose_offset: i8,
    link_note: u8,
) -> Option<CellHit> {
    let assignment = sampler_assignment_at(instrument, intent)?;
    Some(CellHit {
        instrument_slot: slot,
        voice: assignment.sample_slot.min(7) as u8,
        cell: cell_index(intent.x, intent.y),
        pitch_cents: cell_pitch_cents(
            assignment.tune_semis,
            assignment.tune_cents,
            instrument
                .sample_follow_transpose
                .then_some(transpose_offset),
            instrument.sample_follow_pitch.then_some(link_note),
        ),
        velocity: super::sampler_assignment_velocity(velocity, assignment, instrument),
    })
}

pub(in super::super) fn sampler_release_for_intent(
    instrument: &NativeInstrumentSlot,
    slot: u8,
    intent: &CellTriggerIntent,
) -> Option<CellHit> {
    let assignment = sampler_assignment_at(instrument, intent)?;
    Some(cell_release(
        slot,
        assignment.sample_slot.min(7) as u8,
        cell_index(intent.x, intent.y),
    ))
}

pub(in super::super) fn cell_release(slot: u8, voice: u8, cell: u8) -> CellHit {
    CellHit {
        instrument_slot: slot,
        voice,
        cell,
        pitch_cents: 0,
        velocity: 0,
    }
}

fn sampler_assignment_at<'a>(
    instrument: &'a NativeInstrumentSlot,
    intent: &CellTriggerIntent,
) -> Option<&'a NativeSampleAssignment> {
    instrument
        .sample_assignments
        .iter()
        .find(|assignment| assignment.x == intent.x && assignment.y == intent.y)
}

/// Link notes play relative to C4 (MIDI 60), so a cell's stamped tune sets which pitch its
/// sample sounds at C4.
const FOLLOW_PITCH_CENTER_NOTE: i16 = 60;

fn cell_pitch_cents(
    tune_semis: i8,
    tune_cents: i8,
    transpose: Option<i8>,
    link_note: Option<u8>,
) -> i16 {
    let semitones = i16::from(tune_semis)
        + transpose.map_or(0, i16::from)
        + link_note.map_or(0, |note| i16::from(note) - FOLLOW_PITCH_CENTER_NOTE);
    (semitones * 100 + i16::from(tune_cents)).clamp(-CELL_PITCH_CENTS_LIMIT, CELL_PITCH_CENTS_LIMIT)
}

pub(in super::super) fn scale_velocity(level_velocity: u8, source_velocity: u8) -> u8 {
    (((u16::from(level_velocity) * u16::from(source_velocity.clamp(1, 127))) / 127).clamp(1, 127))
        as u8
}
