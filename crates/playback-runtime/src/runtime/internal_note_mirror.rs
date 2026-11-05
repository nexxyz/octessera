use super::{HostAdapter, PlaybackRuntime, RuntimeConfig};
use crate::protocol::{RuntimeAdapterError, RuntimeOperation};
use platform_core::MusicalEvent;
use std::collections::BTreeSet;

/// Held internal-instrument notes copied to MIDI out, keyed by `(channel, note)`. Timed notes
/// are not kept: their scheduled note-offs end them.
pub(super) type MirroredHeldNotes = BTreeSet<(u8, u8)>;

impl PlaybackRuntime {
    /// Copies an internal instrument's event to MIDI out; notes only when Send MIDI is all.
    pub(super) fn mirror_internal_event<H: HostAdapter>(
        &mut self,
        event: MusicalEvent,
        host: &mut H,
    ) -> Result<(), RuntimeAdapterError> {
        match event {
            MusicalEvent::Cc { .. } => {}
            _ if !self.config.midi_out_internal_notes => return Ok(()),
            MusicalEvent::NoteOn {
                channel,
                note,
                duration_ms: None,
                ..
            } => {
                self.mirrored_held_notes.insert((channel, note));
            }
            MusicalEvent::NoteOff { channel, note } => {
                self.mirrored_held_notes.remove(&(channel, note));
            }
            MusicalEvent::NoteOn { .. } => {}
        }
        self.send_musical_event_midi(event, host)
    }

    /// Sends note-offs for the held notes copied to MIDI out once `next` stops copying them.
    pub(super) fn end_mirrored_held_notes<H: HostAdapter>(
        &mut self,
        next: &RuntimeConfig,
        host: &mut H,
    ) -> Result<(), RuntimeAdapterError> {
        if next.midi_out_enabled && next.midi_out_internal_notes {
            return Ok(());
        }
        for (channel, note) in std::mem::take(&mut self.mirrored_held_notes) {
            self.send_midi_message(
                &[0x80 | (channel & 0x0F), note.min(127), 0],
                self.sequenced_at,
                RuntimeOperation::MidiEvent,
                host,
            )?;
        }
        Ok(())
    }
}
