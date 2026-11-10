use super::MidiDto;
use serde::{Serialize, Serializer};

/// The saved MIDI settings: MIDI on or off, the chosen ports, clock and start/stop sync, and
/// which instruments send MIDI. The sync source is the transport's.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeMidiSettings {
    pub(super) enabled: bool,
    pub(super) output_id: Option<String>,
    pub(super) input_id: Option<String>,
    pub(super) clock_out_enabled: bool,
    pub(super) clock_in_enabled: bool,
    pub(super) respond_to_start_stop: bool,
    pub(super) send_instruments: MidiSendInstruments,
}

/// Send MIDI: only the MIDI instruments, or every instrument's notes on its own channel too.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MidiSendInstruments {
    Midi,
    All,
}

impl MidiSendInstruments {
    pub(crate) const ALL: [Self; 2] = [Self::Midi, Self::All];

    pub(crate) const fn wire_name(self) -> &'static str {
        match self {
            Self::Midi => "midi",
            Self::All => "all",
        }
    }

    pub(crate) fn from_wire_name(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|instruments| instruments.wire_name() == value)
    }
}

impl Serialize for MidiSendInstruments {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.wire_name())
    }
}

impl Default for NativeMidiSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            output_id: None,
            input_id: None,
            clock_out_enabled: false,
            clock_in_enabled: false,
            respond_to_start_stop: true,
            send_instruments: MidiSendInstruments::Midi,
        }
    }
}

impl NativeMidiSettings {
    /// Takes every setting the document carries; a null port clears it.
    pub(super) fn apply(&mut self, midi: &MidiDto) {
        if let Some(enabled) = midi.enabled {
            self.enabled = enabled;
        }
        if let Some(id) = &midi.out_id {
            self.output_id.clone_from(id);
        }
        if let Some(id) = &midi.in_id {
            self.input_id.clone_from(id);
        }
        if let Some(enabled) = midi.clock_out_enabled {
            self.clock_out_enabled = enabled;
        }
        if let Some(enabled) = midi.clock_in_enabled {
            self.clock_in_enabled = enabled;
        }
        if let Some(respond) = midi.respond_to_start_stop {
            self.respond_to_start_stop = respond;
        }
        if let Some(instruments) = midi
            .send_instruments
            .as_deref()
            .and_then(MidiSendInstruments::from_wire_name)
        {
            self.send_instruments = instruments;
        }
    }
}
