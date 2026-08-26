use midir::{MidiInputConnection, MidiOutputConnection};
use playback_runtime::{HostAdapter, MidiPort, NativeRunner, PlaybackRuntime};
use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use std::sync::Arc;

use crate::input::MidiMessage;

const MIDI_REALTIME_BUDGET: usize = 32;

pub(crate) trait RuntimeOutputSink: HostAdapter {
    fn dispatch_output(
        &mut self,
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
        output: playback_runtime::RuntimeIngest,
    ) -> Result<(), String>;
}

pub(crate) fn drain_midi_messages<A: HostAdapter + RuntimeOutputSink>(
    midi_rx: &Receiver<MidiMessage>,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut A,
) {
    for _ in 0..MIDI_REALTIME_BUDGET {
        let Ok(MidiMessage::Realtime { bytes }) = midi_rx.try_recv() else {
            break;
        };
        match playback.handle_midi_realtime_bytes_with_output(&bytes, runner, adapter) {
            Ok(output) => {
                if let Err(error) = adapter.dispatch_output(playback, runner, output) {
                    eprintln!("realtime MIDI output processing failed: {error}");
                }
            }
            Err(error) => eprintln!("realtime MIDI handling failed: {error}"),
        }
    }
}

pub(crate) struct MidiHost {
    midi_out: Option<MidiOutputConnection>,
    midi_in: Option<MidiInputConnection<()>>,
    midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
    selected_midi_output_id: Option<String>,
    selected_midi_input_id: Option<String>,
    usb_midi_out_enabled: bool,
}

impl MidiHost {
    pub(crate) fn new(
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
        usb_midi_out_enabled: bool,
    ) -> Self {
        Self {
            midi_out: None,
            midi_in: None,
            midi_in_handler,
            selected_midi_output_id: None,
            selected_midi_input_id: None,
            usb_midi_out_enabled,
        }
    }

    pub(crate) fn list_outputs(&self) -> Result<Vec<MidiPort>, String> {
        let (out, ports) = midi_outputs()?;
        let names = port_names(&out, &ports);
        let ids = stable_port_ids(&names);
        Ok(ids
            .into_iter()
            .zip(names)
            .map(|(id, name)| MidiPort {
                id,
                name: display_midi_port_name(&name),
            })
            .collect())
    }

    pub(crate) fn list_inputs(&self) -> Result<Vec<MidiPort>, String> {
        let (input, ports) = midi_inputs()?;
        let names = port_names(&input, &ports);
        let ids = stable_port_ids(&names);
        Ok(ids
            .into_iter()
            .zip(names)
            .map(|(id, name)| MidiPort {
                id,
                name: display_midi_port_name(&name),
            })
            .collect())
    }

    pub(crate) fn select_output(&mut self, requested: Option<String>) -> Result<(), String> {
        self.midi_out = None;
        self.selected_midi_output_id = None;
        if !self.usb_midi_out_enabled && requested.is_none() {
            return Ok(());
        }
        let (out, ports) = midi_outputs()?;
        let names = port_names(&out, &ports);
        let ids = stable_port_ids(&names);
        let id = if self.usb_midi_out_enabled {
            let index = names
                .iter()
                .position(|name| is_usb_gadget_midi_name(name))
                .ok_or_else(|| "USB MIDI gadget output not found".to_string())?;
            ids[index].clone()
        } else {
            let requested = requested.expect("MIDI output request was checked above");
            resolve_port_id(&requested, &ids)?
        };
        let index = ids
            .iter()
            .position(|candidate| candidate == &id)
            .ok_or_else(|| "MIDI output not found".to_string())?;
        let port = ports
            .get(index)
            .ok_or_else(|| "MIDI output disappeared".to_string())?;
        self.midi_out = Some(
            out.connect(port, "octessera-pi-out")
                .map_err(|error| error.to_string())?,
        );
        self.selected_midi_output_id = Some(id);
        Ok(())
    }

    pub(crate) fn select_input(&mut self, requested: Option<String>) -> Result<(), String> {
        self.midi_in = None;
        self.selected_midi_input_id = None;
        let Some(requested) = requested else {
            return Ok(());
        };
        let (mut input, ports) = midi_inputs()?;
        input.ignore(midir::Ignore::None);
        let names = port_names(&input, &ports);
        let ids = stable_port_ids(&names);
        let id = resolve_port_id(&requested, &ids)?;
        let index = ids
            .iter()
            .position(|candidate| candidate == &id)
            .ok_or_else(|| "MIDI input not found".to_string())?;
        let port = ports
            .get(index)
            .ok_or_else(|| "MIDI input disappeared".to_string())?;
        let handler = self.midi_in_handler.clone();
        self.midi_in = Some(
            input
                .connect(
                    port,
                    "octessera-pi-in",
                    move |_timestamp, message, _| handler(message.to_vec()),
                    (),
                )
                .map_err(|error| error.to_string())?,
        );
        self.selected_midi_input_id = Some(id);
        Ok(())
    }

    pub(crate) fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        let Some(connection) = self.midi_out.as_mut() else {
            return Ok(());
        };
        connection.send(bytes).map_err(|error| error.to_string())
    }

    pub(crate) fn panic(&mut self) -> Result<(), String> {
        let mut first_error = None;
        for bytes in std::iter::once(vec![0xFC]).chain(
            (0..16_u8)
                .flat_map(|channel| [vec![0xB0 | channel, 120, 0], vec![0xB0 | channel, 123, 0]]),
        ) {
            if let Err(error) = self.send(&bytes) {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    pub(crate) fn selected_output_id(&self) -> Option<String> {
        self.selected_midi_output_id.clone()
    }

    pub(crate) fn selected_input_id(&self) -> Option<String> {
        self.selected_midi_input_id.clone()
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    pub(crate) fn usb_midi_out_enabled(&self) -> bool {
        self.usb_midi_out_enabled
    }
}

fn resolve_port_id(requested: &str, ids: &[String]) -> Result<String, String> {
    if ids.iter().any(|id| id == requested) {
        return Ok(requested.into());
    }
    let index = requested
        .parse::<usize>()
        .map_err(|_| "invalid MIDI port id".to_string())?;
    ids.get(index)
        .cloned()
        .ok_or_else(|| "MIDI port not found".to_string())
}

fn stable_port_ids(names: &[String]) -> Vec<String> {
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

fn port_names<T>(host: &T, ports: &[T::Port]) -> Vec<String>
where
    T: MidiPortHost,
{
    ports
        .iter()
        .map(|port| host.port_name(port).unwrap_or_else(|_| "<unknown>".into()))
        .collect()
}

trait MidiPortHost {
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

fn is_usb_gadget_midi_name(name: &str) -> bool {
    let name = name.trim().to_ascii_lowercase();
    name == "octessera midi"
        || name.starts_with("octessera midi:")
        || name.starts_with("octessera midi ")
        || name.contains(":octessera midi ")
        || name == "f_midi"
        || name.starts_with("f_midi:")
        || name.starts_with("f_midi ")
}

fn display_midi_port_name(raw_name: &str) -> String {
    if is_usb_gadget_midi_name(raw_name) {
        "Octessera MIDI".into()
    } else {
        raw_name.into()
    }
}

fn midi_outputs() -> Result<(midir::MidiOutput, Vec<midir::MidiOutputPort>), String> {
    let output = midir::MidiOutput::new("octessera-pi-out").map_err(|error| error.to_string())?;
    let ports = output.ports();
    Ok((output, ports))
}

fn midi_inputs() -> Result<(midir::MidiInput, Vec<midir::MidiInputPort>), String> {
    let input = midir::MidiInput::new("octessera-pi-in").map_err(|error| error.to_string())?;
    let ports = input.ports();
    Ok((input, ports))
}

#[cfg(test)]
mod tests {
    use super::{
        display_midi_port_name, is_usb_gadget_midi_name, resolve_port_id, stable_port_ids,
    };

    #[test]
    fn usb_gadget_midi_names_include_kernel_f_midi_port() {
        assert!(is_usb_gadget_midi_name("f_midi"));
        assert!(is_usb_gadget_midi_name("f_midi 20:0"));
        assert!(is_usb_gadget_midi_name("Octessera MIDI"));
        assert!(is_usb_gadget_midi_name(
            "Octessera MIDI:Octessera MIDI 20:0"
        ));
        assert!(!is_usb_gadget_midi_name("Midi Through Port-0"));
        assert!(!is_usb_gadget_midi_name("Octessera Controller"));
        assert!(!is_usb_gadget_midi_name("UAC2 Gadget MIDI"));
        assert!(!is_usb_gadget_midi_name("MIDI Gadget"));
        assert!(!is_usb_gadget_midi_name("Generic Gadget MIDI"));
        assert!(!is_usb_gadget_midi_name("USB MIDI Controller"));
    }

    #[test]
    fn display_names_normalize_only_octessera_gadget_ports() {
        assert_eq!(
            display_midi_port_name("Octessera MIDI:Octessera MIDI 20:0"),
            "Octessera MIDI"
        );
        assert_eq!(display_midi_port_name("f_midi"), "Octessera MIDI");
        assert_eq!(
            display_midi_port_name("USB MIDI Controller"),
            "USB MIDI Controller"
        );
        assert_eq!(
            display_midi_port_name("Octessera Controller"),
            "Octessera Controller"
        );
    }

    #[test]
    fn port_ids_are_stable_for_reordered_unique_names() {
        let first = stable_port_ids(&["MIDI Through".into(), "Octessera MIDI".into()]);
        let second = stable_port_ids(&["Octessera MIDI".into(), "MIDI Through".into()]);
        assert_eq!(first[0], "name:MIDI Through");
        assert_eq!(first[1], "name:Octessera MIDI");
        assert_eq!(second[0], "name:Octessera MIDI");
        assert_eq!(second[1], "name:MIDI Through");
    }

    #[test]
    fn legacy_index_selection_resolves_to_a_stable_identity() {
        let ids = stable_port_ids(&["MIDI Through".into(), "Octessera MIDI".into()]);
        assert_eq!(resolve_port_id("1", &ids).unwrap(), "name:Octessera MIDI");
        assert_eq!(
            resolve_port_id("name:MIDI Through", &ids).unwrap(),
            "name:MIDI Through"
        );
        assert!(resolve_port_id("name:missing", &ids).is_err());
    }
}
