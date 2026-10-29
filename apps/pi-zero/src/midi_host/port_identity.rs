use playback_runtime::MidiPort;
use std::collections::HashMap;

pub(super) fn resolve_port_id(requested: &str, ids: &[String]) -> Option<String> {
    ids.iter()
        .any(|id| id == requested)
        .then(|| requested.into())
}

pub(super) fn resolve_selected_port_id(
    requested: Option<&str>,
    names: &[String],
    ids: &[String],
    usb_midi_enabled: bool,
    direction: &str,
) -> Result<Option<String>, String> {
    if usb_midi_enabled {
        let index = names
            .iter()
            .position(|name| is_usb_gadget_midi_name(name))
            .ok_or_else(|| format!("USB MIDI gadget {direction} not found"))?;
        return ids
            .get(index)
            .cloned()
            .map(Some)
            .ok_or_else(|| format!("USB MIDI gadget {direction} disappeared"));
    }
    Ok(requested.and_then(|requested| resolve_port_id(requested, ids)))
}

pub(super) fn stable_port_ids(names: &[String]) -> Vec<String> {
    let mut occurrences = HashMap::new();
    names
        .iter()
        .map(|name| {
            let occurrence = occurrences.entry(name.clone()).or_insert(0_usize);
            let id = if *occurrence == 0 {
                format!("name:{name}")
            } else {
                format!("name:{name}#{occurrence}")
            };
            *occurrence += 1;
            id
        })
        .collect()
}

pub(super) fn port_names<T>(host: &T, ports: &[T::Port]) -> Vec<String>
where
    T: MidiPortHost,
{
    ports
        .iter()
        .map(|port| host.port_name(port).unwrap_or_else(|_| "<unknown>".into()))
        .collect()
}

pub(super) trait MidiPortHost {
    type Port;

    fn port_name(&self, port: &Self::Port) -> Result<String, String>;
}

impl MidiPortHost for midir::MidiOutput {
    type Port = midir::MidiOutputPort;

    fn port_name(&self, port: &Self::Port) -> Result<String, String> {
        self.port_name(port).map_err(|error| error.to_string())
    }
}

impl MidiPortHost for midir::MidiInput {
    type Port = midir::MidiInputPort;

    fn port_name(&self, port: &Self::Port) -> Result<String, String> {
        self.port_name(port).map_err(|error| error.to_string())
    }
}

pub(super) fn is_usb_gadget_midi_name(name: &str) -> bool {
    let name = name.trim().to_ascii_lowercase();
    name == "octessera midi"
        || name.starts_with("octessera midi:")
        || name.starts_with("octessera midi ")
        || name.contains(":octessera midi ")
        || name == "f_midi"
        || name.starts_with("f_midi:")
        || name.starts_with("f_midi ")
}

pub(super) fn host_midi_port_names(names: &[String]) -> Vec<String> {
    names
        .iter()
        .filter(|name| !is_usb_gadget_midi_name(name))
        .cloned()
        .collect()
}

pub(super) fn midi_ports_from_names(names: &[String]) -> Vec<MidiPort> {
    let names = host_midi_port_names(names);
    stable_port_ids(&names)
        .into_iter()
        .zip(names)
        .map(|(id, name)| MidiPort { id, name })
        .collect()
}

pub(super) fn midi_outputs() -> Result<(midir::MidiOutput, Vec<midir::MidiOutputPort>), String> {
    let output = midir::MidiOutput::new("octessera-pi-out").map_err(|error| error.to_string())?;
    let ports = output.ports();
    Ok((output, ports))
}

pub(super) fn midi_inputs() -> Result<(midir::MidiInput, Vec<midir::MidiInputPort>), String> {
    let input = midir::MidiInput::new("octessera-pi-in").map_err(|error| error.to_string())?;
    let ports = input.ports();
    Ok((input, ports))
}
