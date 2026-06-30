use super::{NativeInstrumentSlot, NativeMenuModel};

pub(super) fn apply_midi_menu_fields(
    menu: &NativeMenuModel,
    index: usize,
    instrument: &mut NativeInstrumentSlot,
) -> bool {
    let mut changed = false;
    if let Some(enabled) = menu
        .value_for_key(&format!("instruments.{index}.midi.enabled"))
        .map(|value| value == "true")
    {
        if instrument.midi_enabled != enabled {
            instrument.midi_enabled = enabled;
            changed = true;
        }
    }
    if let Some(channel) = menu.number_for_key(&format!("instruments.{index}.midi.channel")) {
        let channel = channel.clamp(1, 16) as u8;
        if instrument.midi_channel != channel {
            instrument.midi_channel = channel;
            changed = true;
        }
    }
    if let Some(velocity) = menu.number_for_key(&format!("instruments.{index}.midi.velocity")) {
        let velocity = velocity.clamp(1, 127) as u8;
        if instrument.midi_velocity != velocity {
            instrument.midi_velocity = velocity;
            changed = true;
        }
    }
    if let Some(duration_ms) = menu.number_for_key(&format!("instruments.{index}.midi.durationMs"))
    {
        let duration_ms = duration_ms.clamp(10, 2000) as u16;
        if instrument.midi_duration_ms != duration_ms {
            instrument.midi_duration_ms = duration_ms;
            changed = true;
        }
    }
    changed
}
