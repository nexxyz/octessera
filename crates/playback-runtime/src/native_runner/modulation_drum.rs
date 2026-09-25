use crate::protocol::DrumHit;
use platform_core::CellTriggerIntent;

pub(in super::super) fn drum_hit_for_intent(
    instruments: &[super::super::NativeInstrumentSlot],
    slot: u8,
    intent: &CellTriggerIntent,
    velocity: u8,
) -> Option<DrumHit> {
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
            cell.get("x").and_then(serde_json::Value::as_u64) == Some(intent.x as u64)
                && cell.get("y").and_then(serde_json::Value::as_u64) == Some(intent.y as u64)
        })?;
    let voice = u8::try_from(assignment.get("voice")?.as_u64()?).ok()?;
    if voice > 7 {
        return None;
    }
    let tune_semis = match assignment.get("tuneSemis") {
        Some(value) => i8::try_from(value.as_i64()?).ok()?,
        None => 0,
    };
    if !(-24..=24).contains(&tune_semis) {
        return None;
    }
    Some(DrumHit {
        instrument_slot: slot,
        voice,
        tune_semis,
        velocity,
    })
}
