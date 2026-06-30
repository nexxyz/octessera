use crate::host_adapter::PiPlaybackHostAdapter;
use playback_runtime::MidiPort;

impl PiPlaybackHostAdapter {
    pub(super) fn list_midi_outputs() -> Result<Vec<MidiPort>, String> {
        let (out, ports) = midi_outputs()?;
        Ok(ports
            .iter()
            .enumerate()
            .map(|(index, port)| MidiPort {
                id: index.to_string(),
                name: out.port_name(port).unwrap_or_else(|_| "<unknown>".into()),
            })
            .collect())
    }

    pub(super) fn list_midi_inputs() -> Result<Vec<MidiPort>, String> {
        let (input, ports) = midi_inputs()?;
        Ok(ports
            .iter()
            .enumerate()
            .map(|(index, port)| MidiPort {
                id: index.to_string(),
                name: input.port_name(port).unwrap_or_else(|_| "<unknown>".into()),
            })
            .collect())
    }

    pub(super) fn select_output(&mut self, id: Option<String>) -> Result<(), String> {
        self.midi_out = None;
        self.selected_midi_output_id = None;
        let Some(id) = id else {
            return Ok(());
        };
        let index = id
            .parse::<usize>()
            .map_err(|_| "invalid MIDI output id".to_string())?;
        let (out, ports) = midi_outputs()?;
        let port = ports
            .get(index)
            .ok_or_else(|| "MIDI output not found".to_string())?;
        self.midi_out = Some(
            out.connect(port, "cellsymphony-pi-out")
                .map_err(|e| e.to_string())?,
        );
        self.selected_midi_output_id = Some(id);
        Ok(())
    }

    pub(super) fn select_input(&mut self, id: Option<String>) -> Result<(), String> {
        self.midi_in = None;
        self.selected_midi_input_id = None;
        let Some(id) = id else {
            return Ok(());
        };
        let index = id
            .parse::<usize>()
            .map_err(|_| "invalid MIDI input id".to_string())?;
        let (mut input, ports) = midi_inputs()?;
        input.ignore(midir::Ignore::None);
        let port = ports
            .get(index)
            .ok_or_else(|| "MIDI input not found".to_string())?;
        let handler = self.midi_in_handler.clone();
        self.midi_in = Some(
            input
                .connect(
                    port,
                    "cellsymphony-pi-in",
                    move |_timestamp, message, _| handler(message.to_vec()),
                    (),
                )
                .map_err(|e| e.to_string())?,
        );
        self.selected_midi_input_id = Some(id);
        Ok(())
    }
}

fn midi_outputs() -> Result<(midir::MidiOutput, Vec<midir::MidiOutputPort>), String> {
    let out = midir::MidiOutput::new("cellsymphony-pi-out").map_err(|e| e.to_string())?;
    let ports = out.ports();
    Ok((out, ports))
}

fn midi_inputs() -> Result<(midir::MidiInput, Vec<midir::MidiInputPort>), String> {
    let input = midir::MidiInput::new("cellsymphony-pi-in").map_err(|e| e.to_string())?;
    let ports = input.ports();
    Ok((input, ports))
}
