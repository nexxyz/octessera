use crate::AppState;
use midir::MidiInputConnection;
use midir::{Ignore, MidiInput, MidiOutput};
use std::sync::{Arc, Mutex};
use tauri::Emitter;

#[derive(Clone, serde::Serialize)]
pub struct MidiPortInfo {
    pub(crate) id: String,
    pub(crate) name: String,
}

#[derive(serde::Serialize, Clone)]
pub(crate) struct MidiInMessage {
    pub(crate) bytes: Vec<u8>,
}

#[tauri::command]
pub fn midi_list_outputs() -> Result<Vec<MidiPortInfo>, String> {
    list_outputs()
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

#[tauri::command]
pub fn midi_list_inputs() -> Result<Vec<MidiPortInfo>, String> {
    list_inputs()
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

#[tauri::command]
pub fn midi_select_output(id: Option<String>, state: tauri::State<AppState>) -> Result<(), String> {
    select_output(id, &state.midi_out)
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

#[tauri::command]
pub fn midi_select_input(
    id: Option<String>,
    state: tauri::State<AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    select_input(id, &state.midi_in, app)
}

pub(crate) fn select_input(
    id: Option<String>,
    midi_in: &Arc<Mutex<Option<MidiInputConnection<()>>>>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    select_input_with_handler(id, midi_in, move |bytes| {
        let _ = app.emit("midi_in", MidiInMessage { bytes });
    })
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

#[tauri::command]
pub fn midi_send(bytes: Vec<u8>, state: tauri::State<AppState>) -> Result<(), String> {
    let mut guard = state
        .midi_out
        .lock()
        .map_err(|_| "midi mutex poisoned".to_string())?;
    let Some(conn) = guard.as_mut() else {
        return Ok(());
    };
    conn.send(&bytes).map_err(|e| e.to_string())
}
