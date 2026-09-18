use midir::{MidiInputConnection, MidiOutputConnection};
use playback_runtime::{HostAdapter, MidiPort, NativeRunner, PlaybackRuntime};
use std::collections::HashMap;
#[cfg(test)]
use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use std::sync::Arc;

use crate::input::MidiMessage;

const MIDI_REALTIME_BUDGET: usize = 32;

#[cfg(test)]
struct TestMidiBackend {
    output_names: Vec<String>,
    input_names: Vec<String>,
    selection_results: VecDeque<Result<(), String>>,
}

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
    host_midi_output_id: Option<String>,
    host_midi_input_id: Option<String>,
    midi_output_error: Option<String>,
    midi_input_error: Option<String>,
    midi_output_attempted: bool,
    midi_input_attempted: bool,
    midi_output_connected: bool,
    midi_input_connected: bool,
    usb_midi_out_enabled: bool,
    #[cfg(test)]
    test_backend: Option<TestMidiBackend>,
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
            host_midi_output_id: None,
            host_midi_input_id: None,
            midi_output_error: None,
            midi_input_error: None,
            midi_output_attempted: false,
            midi_input_attempted: false,
            midi_output_connected: false,
            midi_input_connected: false,
            usb_midi_out_enabled,
            #[cfg(test)]
            test_backend: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_with_test_backend(
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
        usb_midi_out_enabled: bool,
        output_names: impl IntoIterator<Item = String>,
        input_names: impl IntoIterator<Item = String>,
        results: impl IntoIterator<Item = Result<(), String>>,
    ) -> Self {
        let mut host = Self::new(midi_in_handler, usb_midi_out_enabled);
        host.set_test_backend(output_names, input_names, results);
        host
    }

    #[cfg(test)]
    pub(crate) fn set_test_backend(
        &mut self,
        output_names: impl IntoIterator<Item = String>,
        input_names: impl IntoIterator<Item = String>,
        results: impl IntoIterator<Item = Result<(), String>>,
    ) {
        self.test_backend = Some(TestMidiBackend {
            output_names: output_names.into_iter().collect(),
            input_names: input_names.into_iter().collect(),
            selection_results: results.into_iter().collect(),
        });
    }

    pub(crate) fn list_outputs(&self) -> Result<Vec<MidiPort>, String> {
        #[cfg(test)]
        if let Some(backend) = &self.test_backend {
            return Ok(midi_ports_from_names(&backend.output_names));
        }
        let (out, ports) = midi_outputs()?;
        Ok(midi_ports_from_names(&port_names(&out, &ports)))
    }

    pub(crate) fn list_inputs(&self) -> Result<Vec<MidiPort>, String> {
        #[cfg(test)]
        if let Some(backend) = &self.test_backend {
            return Ok(midi_ports_from_names(&backend.input_names));
        }
        let (input, ports) = midi_inputs()?;
        Ok(midi_ports_from_names(&port_names(&input, &ports)))
    }

    pub(crate) fn select_output(&mut self, requested: Option<String>) -> Result<(), String> {
        self.midi_out = None;
        self.host_midi_output_id = requested.clone();
        self.midi_output_error = None;
        self.midi_output_attempted = true;
        self.midi_output_connected = false;
        if !self.usb_midi_out_enabled && requested.is_none() {
            return Ok(());
        }
        let result = self
            .take_test_selection_result(requested.as_deref(), "output")
            .unwrap_or_else(|| self.select_output_native(requested.as_deref()));
        self.midi_output_connected = result.is_ok();
        if let Err(error) = &result {
            self.midi_output_error = Some(error.clone());
        }
        result
    }

    pub(crate) fn select_input(&mut self, requested: Option<String>) -> Result<(), String> {
        self.midi_in = None;
        self.host_midi_input_id = requested.clone();
        self.midi_input_error = None;
        self.midi_input_attempted = true;
        self.midi_input_connected = false;
        if !self.usb_midi_out_enabled && requested.is_none() {
            return Ok(());
        }
        let result = self
            .take_test_selection_result(requested.as_deref(), "input")
            .unwrap_or_else(|| self.select_input_native(requested.as_deref()));
        self.midi_input_connected = result.is_ok();
        if let Err(error) = &result {
            self.midi_input_error = Some(error.clone());
        }
        result
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
        self.host_midi_output_id.clone()
    }

    pub(crate) fn selected_input_id(&self) -> Option<String> {
        self.host_midi_input_id.clone()
    }

    pub(crate) fn selection_status(
        &self,
        result: Result<(), String>,
    ) -> Option<(bool, Option<String>)> {
        if !self.usb_midi_out_enabled {
            return Some(result.map_or_else(|error| (false, Some(error)), |_| (true, None)));
        }
        if let Some(message) = self
            .midi_output_error
            .as_deref()
            .or(self.midi_input_error.as_deref())
            .map(str::to_string)
            .or_else(|| result.err())
        {
            return Some((false, Some(message)));
        }
        if !self.midi_output_attempted || !self.midi_input_attempted {
            return None;
        }
        let message = usb_midi_route_error(
            self.midi_output_connected,
            self.midi_input_connected,
            None,
            None,
        );
        Some((message.is_none(), message))
    }

    fn select_output_native(&mut self, requested: Option<&str>) -> Result<(), String> {
        let (out, ports) = midi_outputs()?;
        let names = port_names(&out, &ports);
        let ids = stable_port_ids(&names);
        let Some(id) =
            resolve_selected_port_id(requested, &names, &ids, self.usb_midi_out_enabled, "output")?
        else {
            return Ok(());
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
        Ok(())
    }

    fn select_input_native(&mut self, requested: Option<&str>) -> Result<(), String> {
        let (mut input, ports) = midi_inputs()?;
        input.ignore(midir::Ignore::None);
        let names = port_names(&input, &ports);
        let ids = stable_port_ids(&names);
        let Some(id) =
            resolve_selected_port_id(requested, &names, &ids, self.usb_midi_out_enabled, "input")?
        else {
            return Ok(());
        };
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
        Ok(())
    }

    fn take_test_selection_result(
        &mut self,
        requested: Option<&str>,
        direction: &str,
    ) -> Option<Result<(), String>> {
        #[cfg(test)]
        {
            let resolution = {
                let backend = self.test_backend.as_ref()?;
                let names = if direction == "output" {
                    &backend.output_names
                } else {
                    &backend.input_names
                };
                let ids = stable_port_ids(names);
                resolve_selected_port_id(
                    requested,
                    names,
                    &ids,
                    self.usb_midi_out_enabled,
                    direction,
                )
            };
            Some(match resolution {
                Ok(None) => Ok(()),
                Ok(Some(_)) => self
                    .test_backend
                    .as_mut()
                    .and_then(|backend| backend.selection_results.pop_front())
                    .unwrap_or_else(|| Err("test MIDI connection result missing".into())),
                Err(error) => Err(error),
            })
        }
        #[cfg(not(test))]
        {
            let _ = (requested, direction);
            None
        }
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

fn resolve_selected_port_id(
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
    requested
        .map(|requested| resolve_port_id(requested, ids))
        .transpose()
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

fn host_midi_port_names(names: &[String]) -> Vec<String> {
    names
        .iter()
        .filter(|name| !is_usb_gadget_midi_name(name))
        .cloned()
        .collect()
}

fn midi_ports_from_names(names: &[String]) -> Vec<MidiPort> {
    let names = host_midi_port_names(names);
    stable_port_ids(&names)
        .into_iter()
        .zip(names)
        .map(|(id, name)| MidiPort { id, name })
        .collect()
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

fn usb_midi_route_error(
    output_connected: bool,
    input_connected: bool,
    output_error: Option<&str>,
    input_error: Option<&str>,
) -> Option<String> {
    output_error
        .map(str::to_string)
        .or_else(|| (!output_connected).then(|| "USB MIDI gadget output not connected".into()))
        .or_else(|| input_error.map(str::to_string))
        .or_else(|| (!input_connected).then(|| "USB MIDI gadget input not connected".into()))
}

#[cfg(test)]
#[path = "midi_host_startup_tests.rs"]
mod startup_tests;
#[cfg(test)]
#[path = "midi_host_tests.rs"]
mod tests;
