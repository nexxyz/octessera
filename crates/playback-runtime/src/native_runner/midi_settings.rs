use super::MidiDto;

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
    pub(super) send_instruments: String,
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
            send_instruments: "midi".into(),
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
        if let Some(instruments) = &midi.send_instruments {
            self.send_instruments.clone_from(instruments);
        }
    }
}
