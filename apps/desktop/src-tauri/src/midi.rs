use midir::MidiInputConnection;
use midir::{Ignore, MidiInput, MidiOutput};
use playback_runtime::TimedMidiSender;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, serde::Serialize)]
pub struct MidiPortInfo {
    pub(crate) id: String,
    pub(crate) name: String,
}

pub(crate) fn list_outputs() -> Result<Vec<MidiPortInfo>, String> {
    let out = MidiOutput::new("octessera-midi-out").map_err(|e| e.to_string())?;
    let names = port_names(&out.ports(), |port| out.port_name(port));
    Ok(port_infos(names))
}

pub(crate) fn list_inputs() -> Result<Vec<MidiPortInfo>, String> {
    let mut input = MidiInput::new("octessera-midi-in").map_err(|e| e.to_string())?;
    input.ignore(Ignore::None);
    let names = port_names(&input.ports(), |port| input.port_name(port));
    Ok(port_infos(names))
}

/// Connects the port saved as `id`; `Ok(false)` when it is not plugged in.
pub(crate) fn select_output(
    id: Option<String>,
    midi_out: &mut TimedMidiSender,
) -> Result<bool, String> {
    midi_out.set_sink(None)?;
    let Some(id) = id else {
        return Ok(true);
    };
    let out = MidiOutput::new("octessera-midi-out").map_err(|e| e.to_string())?;
    let ports = out.ports();
    let names = port_names(&ports, |port| out.port_name(port));
    let Some(port) = port_position(&id, &names).map(|index| &ports[index]) else {
        return Ok(false);
    };
    let mut conn = out
        .connect(port, "octessera-midi-out-conn")
        .map_err(|e| e.to_string())?;
    midi_out.set_sink(Some(Box::new(move |bytes| {
        conn.send(bytes).map_err(|e| e.to_string())
    })))?;
    Ok(true)
}

/// Connects the port saved as `id`; `Ok(false)` when it is not plugged in.
pub(crate) fn select_input_with_handler<F>(
    id: Option<String>,
    midi_in: &Arc<Mutex<Option<MidiInputConnection<()>>>>,
    mut handler: F,
) -> Result<bool, String>
where
    F: FnMut(Vec<u8>) + Send + 'static,
{
    let mut guard = midi_in
        .lock()
        .map_err(|_| "midi mutex poisoned".to_string())?;
    *guard = None;
    let Some(id) = id else {
        return Ok(true);
    };
    let mut input = MidiInput::new("octessera-midi-in").map_err(|e| e.to_string())?;
    input.ignore(Ignore::None);
    let ports = input.ports();
    let names = port_names(&ports, |port| input.port_name(port));
    let Some(port) = port_position(&id, &names).map(|index| &ports[index]) else {
        return Ok(false);
    };
    let conn = input
        .connect(
            port,
            "octessera-midi-in-conn",
            move |_stamp, msg, _| {
                handler(msg.to_vec());
            },
            (),
        )
        .map_err(|e| e.to_string())?;
    *guard = Some(conn);
    Ok(true)
}

fn port_names<P, E>(ports: &[P], name: impl Fn(&P) -> Result<String, E>) -> Vec<String> {
    ports
        .iter()
        .map(|port| name(port).unwrap_or_else(|_| "<unknown>".to_string()))
        .collect()
}

fn port_infos(names: Vec<String>) -> Vec<MidiPortInfo> {
    port_ids(&names)
        .into_iter()
        .zip(names)
        .map(|(id, name)| MidiPortInfo { id, name })
        .collect()
}

/// The boards' port IDs: the name, with `#n` for the n-th repeat of a name.
fn port_ids(names: &[String]) -> Vec<String> {
    let mut occurrences = HashMap::new();
    names
        .iter()
        .map(|name| {
            let occurrence = occurrences.entry(name.as_str()).or_insert(0_usize);
            let id = match *occurrence {
                0 => format!("name:{name}"),
                repeat => format!("name:{name}#{repeat}"),
            };
            *occurrence += 1;
            id
        })
        .collect()
}

fn port_position(id: &str, names: &[String]) -> Option<usize> {
    port_ids(names).iter().position(|port_id| port_id == id)
}

#[cfg(test)]
mod tests;
