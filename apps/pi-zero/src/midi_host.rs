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
    host_midi_output_id: Option<String>,
    host_midi_input_id: Option<String>,
    midi_output_error: Option<String>,
    midi_input_error: Option<String>,
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
            host_midi_output_id: None,
            host_midi_input_id: None,
            midi_output_error: None,
            midi_input_error: None,
            usb_midi_out_enabled,
        }
    }

    pub(crate) fn list_outputs(&self) -> Result<Vec<MidiPort>, String> {
        let (out, ports) = midi_outputs()?;
        let names = host_midi_port_names(&port_names(&out, &ports));
        let ids = stable_port_ids(&names);
        Ok(ids
            .into_iter()
            .zip(names)
            .map(|(id, name)| MidiPort { id, name })
            .collect())
    }

    pub(crate) fn list_inputs(&self) -> Result<Vec<MidiPort>, String> {
        let (input, ports) = midi_inputs()?;
        let names = host_midi_port_names(&port_names(&input, &ports));
        let ids = stable_port_ids(&names);
        Ok(ids
            .into_iter()
            .zip(names)
            .map(|(id, name)| MidiPort { id, name })
            .collect())
    }

    pub(crate) fn select_output(&mut self, requested: Option<String>) -> Result<(), String> {
        self.midi_out = None;
        self.host_midi_output_id = requested.clone();
        self.midi_output_error = None;
        if !self.usb_midi_out_enabled && requested.is_none() {
            return Ok(());
        }
        let result: Result<(), String> = (|| {
            let (out, ports) = midi_outputs()?;
            let names = port_names(&out, &ports);
            let ids = stable_port_ids(&names);
            let Some(id) = resolve_selected_port_id(
                requested.as_deref(),
                &names,
                &ids,
                self.usb_midi_out_enabled,
                "output",
            )?
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
        })();
        if let Err(error) = &result {
            self.midi_output_error = Some(error.clone());
        }
        result
    }

    pub(crate) fn select_input(&mut self, requested: Option<String>) -> Result<(), String> {
        self.midi_in = None;
        self.host_midi_input_id = requested.clone();
        self.midi_input_error = None;
        if !self.usb_midi_out_enabled && requested.is_none() {
            return Ok(());
        }
        let result: Result<(), String> = (|| {
            let (mut input, ports) = midi_inputs()?;
            input.ignore(midir::Ignore::None);
            let names = port_names(&input, &ports);
            let ids = stable_port_ids(&names);
            let Some(id) = resolve_selected_port_id(
                requested.as_deref(),
                &names,
                &ids,
                self.usb_midi_out_enabled,
                "input",
            )?
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
        })();
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

    pub(crate) fn selection_status(&self, result: Result<(), String>) -> (bool, Option<String>) {
        if !self.usb_midi_out_enabled {
            return result.map_or_else(|error| (false, Some(error)), |_| (true, None));
        }
        let message = usb_midi_route_error(
            self.midi_out.is_some(),
            self.midi_in.is_some(),
            self.midi_output_error.as_deref(),
            self.midi_input_error.as_deref(),
        )
        .or_else(|| result.err());
        (message.is_none(), message)
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
mod tests {
    use super::{
        host_midi_port_names, is_usb_gadget_midi_name, resolve_port_id, resolve_selected_port_id,
        stable_port_ids, usb_midi_route_error,
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
    fn host_lists_filter_recognized_gadget_ports_in_both_directions() {
        let output = vec![
            "Host Output".into(),
            "f_midi 20:0".into(),
            "Octessera MIDI:Octessera MIDI 20:0".into(),
        ];
        assert_eq!(host_midi_port_names(&output), vec!["Host Output"]);
        assert_eq!(
            stable_port_ids(&host_midi_port_names(&output)),
            vec!["name:Host Output"]
        );
        let input = vec![
            "Host Input".into(),
            "f_midi".into(),
            "Octessera MIDI".into(),
        ];
        assert_eq!(host_midi_port_names(&input), vec!["Host Input"]);
        assert_eq!(
            stable_port_ids(&host_midi_port_names(&input)),
            vec!["name:Host Input"]
        );
    }

    #[test]
    fn gadget_output_failure_keeps_pair_status_failed_when_input_connects() {
        assert_eq!(
            usb_midi_route_error(false, true, Some("output failed"), None),
            Some("output failed".into())
        );
        assert_eq!(usb_midi_route_error(true, true, None, None), None);
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

    #[test]
    fn gadget_input_auto_selection_uses_the_gadget_endpoint() {
        let names = vec!["Host Input".into(), "f_midi 20:0".into()];
        let ids = stable_port_ids(&names);
        assert_eq!(
            resolve_selected_port_id(None, &names, &ids, true, "input").unwrap(),
            Some("name:f_midi 20:0".into())
        );
    }

    #[test]
    fn gadget_endpoint_missing_reports_the_required_direction() {
        let names = vec!["Host Input".into()];
        let ids = stable_port_ids(&names);
        for (direction, message) in [
            ("input", "USB MIDI gadget input not found"),
            ("output", "USB MIDI gadget output not found"),
        ] {
            assert_eq!(
                resolve_selected_port_id(Some("name:Host Input"), &names, &ids, true, direction),
                Err(message.into())
            );
        }
    }

    #[test]
    fn normal_host_input_selection_stays_requested() {
        let names = vec!["Host Input".into(), "f_midi 20:0".into()];
        let ids = stable_port_ids(&names);
        assert_eq!(
            resolve_selected_port_id(Some("name:Host Input"), &names, &ids, false, "input")
                .unwrap(),
            Some("name:Host Input".into())
        );
    }

    #[test]
    fn gadget_selection_takes_precedence_for_input_and_output() {
        let names = vec!["Host Port".into(), "Octessera MIDI".into()];
        let ids = stable_port_ids(&names);
        for direction in ["input", "output"] {
            assert_eq!(
                resolve_selected_port_id(Some("name:Host Port"), &names, &ids, true, direction)
                    .unwrap(),
                Some("name:Octessera MIDI".into())
            );
        }
    }
}
