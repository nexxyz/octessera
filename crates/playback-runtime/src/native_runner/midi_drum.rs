//! MIDI Drum: an instrument slot whose stamped tiles send MIDI notes instead of sound.

use super::grid_assign::GridAssign;
use super::led_color::LedColor;
use super::tile_stamp::toggle_tile_stamps;
use super::{NativeInstrumentSlot, NativeRunner, Value};
use crate::native_menu::NativeMenuMidiDrum;
use platform_core::{MusicalEvent, TileTriggerIntent};
use serde_json::json;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct NativeMidiDrum {
    pub(super) settings: NativeMenuMidiDrum,
    pub(super) assignments: Vec<NativeMidiDrumTile>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NativeMidiDrumTile {
    pub(super) x: usize,
    pub(super) y: usize,
    pub(super) note: u8,
    pub(super) velocity: u8,
}

impl NativeMidiDrum {
    pub(super) fn payload(&self) -> Value {
        let settings = &self.settings;
        json!({
            "enabled": settings.enabled,
            "channel": settings.channel,
            "durationMs": settings.duration_ms,
            "tileNote": settings.tile_note,
            "tileVelocity": settings.tile_velocity,
            "assignments": self.assignments.iter().map(|tile| json!({
                "x": tile.x,
                "y": tile.y,
                "note": tile.note,
                "velocity": tile.velocity,
            })).collect::<Vec<_>>(),
        })
    }

    pub(super) fn apply_payload(&mut self, value: &Value) {
        let number = |key: &str| value.get(key).and_then(Value::as_u64);
        let settings = &mut self.settings;
        if let Some(enabled) = value.get("enabled").and_then(Value::as_bool) {
            settings.enabled = enabled;
        }
        if let Some(channel) = number("channel") {
            settings.channel = channel.clamp(1, 16) as u8;
        }
        if let Some(duration_ms) = number("durationMs") {
            settings.duration_ms = duration_ms.clamp(10, 2000) as u16;
        }
        if let Some(note) = number("tileNote") {
            settings.tile_note = note.min(127) as u8;
        }
        if let Some(velocity) = number("tileVelocity") {
            settings.tile_velocity = velocity.clamp(1, 127) as u8;
        }
        if let Some(assignments) = value.get("assignments").and_then(Value::as_array) {
            self.assignments = assignments
                .iter()
                .filter_map(|tile| {
                    let field = |key: &str| tile.get(key).and_then(Value::as_u64);
                    Some(NativeMidiDrumTile {
                        x: field("x")? as usize,
                        y: field("y")? as usize,
                        note: field("note")?.min(127) as u8,
                        velocity: field("velocity")?.clamp(1, 127) as u8,
                    })
                })
                .collect();
        }
    }

    fn stamp_at(&self, x: usize, y: usize) -> NativeMidiDrumTile {
        NativeMidiDrumTile {
            x,
            y,
            note: self.settings.tile_note,
            velocity: self.settings.tile_velocity,
        }
    }
}

fn set_clamped<T: TryFrom<i32>>(field: &mut T, value: i32, min: i32, max: i32) {
    if let Ok(value) = T::try_from(value.clamp(min, max)) {
        *field = value;
    }
}

/// The output channels of the enabled MIDI Drum slots, one bit per channel.
pub(super) fn midi_drum_channels(instruments: &[NativeInstrumentSlot]) -> u16 {
    instruments
        .iter()
        .filter(|instrument| instrument.kind == "midi_drum")
        .filter_map(NativeInstrumentSlot::midi_output_channel)
        .fold(0, |mask, channel| mask | 1 << channel)
}

impl NativeInstrumentSlot {
    /// The 0-based channel a MIDI Inst. or MIDI Drum slot sends on while it is enabled.
    pub(super) fn midi_output_channel(&self) -> Option<u8> {
        let (enabled, channel) = match self.kind.as_str() {
            "midi" => (self.midi_enabled, self.midi_channel),
            "midi_drum" => (
                self.midi_drum.settings.enabled,
                self.midi_drum.settings.channel,
            ),
            _ => return None,
        };
        enabled.then(|| channel.saturating_sub(1).min(15))
    }

    /// The note an enabled MIDI Drum sends when Link strikes a stamped tile; unstamped tiles
    /// send nothing.
    pub(super) fn midi_drum_note_on(&self, intent: &TileTriggerIntent) -> Option<MusicalEvent> {
        let channel = self.midi_output_channel()?;
        let tile = self
            .midi_drum
            .assignments
            .iter()
            .find(|tile| (tile.x, tile.y) == (intent.x, intent.y))?;
        Some(MusicalEvent::NoteOn {
            channel,
            note: tile.note,
            velocity: tile.velocity,
            duration_ms: Some(u32::from(self.midi_drum.settings.duration_ms)),
        })
    }
}

impl NativeRunner {
    pub(super) fn enter_midi_drum_assign(&mut self, slot: usize) {
        let Some(settings) = self
            .instruments
            .get(slot)
            .filter(|instrument| instrument.kind == "midi_drum")
            .map(|instrument| instrument.midi_drum.settings)
        else {
            return;
        };
        self.grid_assign = Some(GridAssign::MidiDrum { slot });
        self.show_toast(format!(
            "N{} v{}: press tiles to assign",
            settings.tile_note, settings.tile_velocity
        ));
    }

    pub(super) fn fast_midi_drum_key(&mut self, slot: usize, key: &str, field: &str) -> bool {
        let number = self.menu.number_for_key(key);
        let enabled = self.menu.value_for_key(key).map(|value| value == "true");
        let Some(settings) = self
            .instruments
            .get_mut(slot)
            .map(|instrument| &mut instrument.midi_drum.settings)
        else {
            return false;
        };
        let before = *settings;
        match (field, number) {
            ("midiDrum.enabled", _) => settings.enabled = enabled.unwrap_or(settings.enabled),
            ("midiDrum.channel", Some(value)) => set_clamped(&mut settings.channel, value, 1, 16),
            ("midiDrum.durationMs", Some(value)) => {
                set_clamped(&mut settings.duration_ms, value, 10, 2000);
            }
            ("midiDrum.tileNote", Some(value)) => {
                set_clamped(&mut settings.tile_note, value, 0, 127)
            }
            ("midiDrum.tileVelocity", Some(value)) => {
                set_clamped(&mut settings.tile_velocity, value, 1, 127);
            }
            _ => return false,
        }
        let after = *settings;
        if (after.enabled, after.channel) != (before.enabled, before.channel) {
            self.menu.show_instrument_label(&self.menu_config(), slot);
        }
        if after != before {
            self.mark_fast_autosave_dirty();
        }
        true
    }

    /// Stamps the Note and Tile Vel onto the pressed tile, or with Shift its row and with
    /// Shift+Fn its column; pressing a line that already carries exactly that stamp clears it.
    pub(super) fn handle_midi_drum_assignment_grid_press(
        &mut self,
        slot: usize,
        x: usize,
        y: usize,
    ) {
        let (tiles, line) = self.stamp_line_tiles(x, y);
        let Some(drum) = self
            .instruments
            .get_mut(slot)
            .filter(|instrument| instrument.kind == "midi_drum" && !tiles.is_empty())
            .map(|instrument| &mut instrument.midi_drum)
        else {
            return;
        };
        let settings = drum.settings;
        let stamped = |x, y| NativeMidiDrumTile {
            x,
            y,
            note: settings.tile_note,
            velocity: settings.tile_velocity,
        };
        let clear = toggle_tile_stamps(
            &mut drum.assignments,
            &tiles,
            |tile| Some((tile.x, tile.y)),
            |tile, x, y| *tile == stamped(x, y),
            stamped,
        );
        self.show_stamp_toast(
            clear,
            line,
            None,
            format!("N{} v{}", settings.tile_note, settings.tile_velocity),
        );
        self.mark_config_dirty();
    }

    /// Tiles with the current stamp light white, other stamped tiles dim, empty tiles stay dark.
    pub(super) fn apply_midi_drum_assignment_overlay(&self, slot: usize, leds: &mut [LedColor]) {
        self.fill_leds(leds, LedColor::BLACK);
        let Some(drum) = self
            .instruments
            .get(slot)
            .map(|instrument| &instrument.midi_drum)
        else {
            return;
        };
        for tile in &drum.assignments {
            let color = if *tile == drum.stamp_at(tile.x, tile.y) {
                LedColor::WHITE
            } else {
                LedColor::WHITE.dim(3)
            };
            self.set_display_led(leds, tile.x, tile.y, color);
        }
    }
}
