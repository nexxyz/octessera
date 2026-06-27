use midir::MidiInputConnection;
use midir::{Ignore, MidiInput, MidiOutput};
use std::sync::{Arc, Mutex};

#[derive(Clone, serde::Serialize)]
pub struct MidiPortInfo {
    pub(crate) id: String,
    pub(crate) name: String,
}

pub(crate) fn list_outputs() -> Result<Vec<MidiPortInfo>, String> {
    let out = MidiOutput::new("cellsymphony-midi-out").map_err(|e| e.to_string())?;
    let ports = out.ports();
    let mut res = Vec::new();
    for (idx, port) in ports.iter().enumerate() {
        let name = out
            .port_name(port)
            .unwrap_or_else(|_| "<unknown>".to_string());
        res.push(MidiPortInfo {
            id: idx.to_string(),
            name,
        });
    }
    Ok(res)
}

pub(crate) fn list_inputs() -> Result<Vec<MidiPortInfo>, String> {
    let mut input = MidiInput::new("cellsymphony-midi-in").map_err(|e| e.to_string())?;
    input.ignore(Ignore::None);
    let ports = input.ports();
    let mut res = Vec::new();
    for (idx, port) in ports.iter().enumerate() {
        let name = input
            .port_name(port)
            .unwrap_or_else(|_| "<unknown>".to_string());
        res.push(MidiPortInfo {
            id: idx.to_string(),
            name,
        });
    }
    Ok(res)
}

pub(crate) fn select_output(
    id: Option<String>,
    midi_out: &Arc<Mutex<Option<midir::MidiOutputConnection>>>,
) -> Result<(), String> {
    let mut guard = midi_out
        .lock()
        .map_err(|_| "midi mutex poisoned".to_string())?;
    *guard = None;
    let Some(id) = id else {
        return Ok(());
    };
    let idx: usize = id
        .parse()
        .map_err(|_| "invalid midi output id".to_string())?;
    let out = MidiOutput::new("cellsymphony-midi-out").map_err(|e| e.to_string())?;
    let ports = out.ports();
    let port = ports
        .get(idx)
        .ok_or_else(|| "midi output id out of range".to_string())?;
    let conn = out
        .connect(port, "cellsymphony-midi-out-conn")
        .map_err(|e| e.to_string())?;
    *guard = Some(conn);
    Ok(())
}

pub(crate) fn select_input_with_handler<F>(
    id: Option<String>,
    midi_in: &Arc<Mutex<Option<MidiInputConnection<()>>>>,
    mut handler: F,
) -> Result<(), String>
where
    F: FnMut(Vec<u8>) + Send + 'static,
{
    let mut guard = midi_in
        .lock()
        .map_err(|_| "midi mutex poisoned".to_string())?;
    *guard = None;
    let Some(id) = id else {
        return Ok(());
    };
    let idx: usize = id
        .parse()
        .map_err(|_| "invalid midi input id".to_string())?;
    let mut input = MidiInput::new("cellsymphony-midi-in").map_err(|e| e.to_string())?;
    input.ignore(Ignore::None);
    let ports = input.ports();
    let port = ports
        .get(idx)
        .ok_or_else(|| "midi input id out of range".to_string())?;
    let conn = input
        .connect(
            port,
            "cellsymphony-midi-in-conn",
            move |_stamp, msg, _| {
                handler(msg.to_vec());
            },
            (),
        )
        .map_err(|e| e.to_string())?;
    *guard = Some(conn);
    Ok(())
}
