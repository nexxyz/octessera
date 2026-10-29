use midir::MidiInputConnection;
use playback_runtime::{HostAdapter, MidiPort, NativeRunner, PlaybackRuntime, TimedMidiSender};
#[cfg(test)]
use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Instant;

use crate::input::MidiMessage;
use port_identity::{
    midi_inputs, midi_outputs, midi_ports_from_names, port_names, resolve_selected_port_id,
    stable_port_ids,
};
use port_watch::PortWatch;

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
        let Ok(MidiMessage::Realtime { bytes, received_at }) = midi_rx.try_recv() else {
            break;
        };
        match playback.handle_midi_realtime_bytes_music_first_with_output(
            &bytes,
            Some(received_at),
            runner,
            adapter,
        ) {
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
    midi_out: TimedMidiSender,
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
    port_watch: PortWatch,
    #[cfg(test)]
    test_backend: Option<TestMidiBackend>,
}

impl MidiHost {
    pub(crate) fn new(
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
        usb_midi_out_enabled: bool,
    ) -> Self {
        Self {
            midi_out: TimedMidiSender::new("octessera-midi-out", midi_sender_start_hook),
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
            port_watch: PortWatch::default(),
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
        self.midi_out.set_sink(None)?;
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
        self.midi_output_connected = result == Ok(true);
        if result == Ok(false) {
            log_unplugged_port("output", requested.as_deref());
        }
        if let Err(error) = &result {
            self.midi_output_error = Some(error.clone());
        }
        result.map(|_| ())
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
        self.midi_input_connected = result == Ok(true);
        if result == Ok(false) {
            log_unplugged_port("input", requested.as_deref());
        }
        if let Err(error) = &result {
            self.midi_input_error = Some(error.clone());
        }
        result.map(|_| ())
    }

    /// Reconnects the remembered MIDI Host ports after a device was plugged in or
    /// unplugged. The USB MIDI gadget port exists for as long as the board runs.
    pub(crate) fn reconnect_if_ports_changed(&mut self, now: Instant) {
        if self.usb_midi_out_enabled || !self.port_watch.changed(now) {
            return;
        }
        if let Some(id) = self.host_midi_output_id.clone() {
            if let Err(error) = self.select_output(Some(id)) {
                eprintln!("MIDI output reconnect failed: {error}");
            }
        }
        if let Some(id) = self.host_midi_input_id.clone() {
            if let Err(error) = self.select_input(Some(id)) {
                eprintln!("MIDI input reconnect failed: {error}");
            }
        }
    }

    pub(crate) fn send(&mut self, at: Option<Instant>, bytes: &[u8]) -> Result<(), String> {
        self.midi_out.send(at, bytes)
    }

    pub(crate) fn flush(&mut self) -> Result<(), String> {
        self.midi_out.flush()
    }

    pub(crate) fn panic(&mut self) -> Result<(), String> {
        self.midi_out.panic()
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

    fn select_output_native(&mut self, requested: Option<&str>) -> Result<bool, String> {
        let (out, ports) = midi_outputs()?;
        let names = port_names(&out, &ports);
        let ids = stable_port_ids(&names);
        let Some(id) =
            resolve_selected_port_id(requested, &names, &ids, self.usb_midi_out_enabled, "output")?
        else {
            return Ok(false);
        };
        let index = ids
            .iter()
            .position(|candidate| candidate == &id)
            .ok_or_else(|| "MIDI output not found".to_string())?;
        let port = ports
            .get(index)
            .ok_or_else(|| "MIDI output disappeared".to_string())?;
        let mut connection = out
            .connect(port, "octessera-pi-out")
            .map_err(|error| error.to_string())?;
        self.midi_out.set_sink(Some(Box::new(move |bytes| {
            connection.send(bytes).map_err(|error| error.to_string())
        })))?;
        Ok(true)
    }

    fn select_input_native(&mut self, requested: Option<&str>) -> Result<bool, String> {
        let (mut input, ports) = midi_inputs()?;
        input.ignore(midir::Ignore::None);
        let names = port_names(&input, &ports);
        let ids = stable_port_ids(&names);
        let Some(id) =
            resolve_selected_port_id(requested, &names, &ids, self.usb_midi_out_enabled, "input")?
        else {
            return Ok(false);
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
        Ok(true)
    }

    fn take_test_selection_result(
        &mut self,
        requested: Option<&str>,
        direction: &str,
    ) -> Option<Result<bool, String>> {
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
                Ok(None) => Ok(false),
                Ok(Some(_)) => self
                    .test_backend
                    .as_mut()
                    .and_then(|backend| backend.selection_results.pop_front())
                    .unwrap_or_else(|| Err("test MIDI connection result missing".into()))
                    .map(|()| true),
                Err(error) => Err(error),
            })
        }
        #[cfg(not(test))]
        {
            let _ = (requested, direction);
            None
        }
    }
}

fn log_unplugged_port(direction: &str, requested: Option<&str>) {
    if let Some(id) = requested {
        eprintln!("MIDI {direction} {id} is not plugged in; it connects when it appears");
    }
}

fn midi_sender_start_hook() {
    #[cfg(any(
        feature = "hardware-raspberry-pi-zero-2w",
        feature = "hardware-orange-pi-zero-2w"
    ))]
    crate::audio_priority::pin_midi_sender_thread();
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

mod port_identity;
mod port_watch;
#[cfg(test)]
mod startup_tests;
#[cfg(test)]
mod tests;
