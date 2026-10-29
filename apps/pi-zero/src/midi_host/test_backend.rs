//! A fake MIDI backend for the connection worker tests: fixed port names and
//! queued connect results instead of ALSA.

use super::port_identity::{resolve_selected_port_id, stable_port_ids};
use playback_runtime::MidiSink;
use std::collections::VecDeque;

pub(super) struct TestMidiBackend {
    pub(super) output_names: Vec<String>,
    pub(super) input_names: Vec<String>,
    pub(super) selection_results: VecDeque<Result<(), String>>,
}

impl TestMidiBackend {
    pub(super) fn take_selection_result(
        &mut self,
        requested: Option<&str>,
        direction: &str,
        usb_midi_out_enabled: bool,
    ) -> Result<bool, String> {
        let names = if direction == "output" {
            &self.output_names
        } else {
            &self.input_names
        };
        let ids = stable_port_ids(names);
        match resolve_selected_port_id(requested, names, &ids, usb_midi_out_enabled, direction)? {
            None => Ok(false),
            Some(_) => self
                .selection_results
                .pop_front()
                .unwrap_or_else(|| Err("test MIDI connection result missing".into()))
                .map(|()| true),
        }
    }
}

pub(super) fn test_sink() -> MidiSink {
    Box::new(|_| Ok(()))
}
