//! The MIDI connection worker: port listing, connecting and the port watch
//! run here, because creating an ALSA sequencer client and connecting a port
//! takes tens of milliseconds on the boards. The runtime thread only sends
//! requests and installs the output sink the worker hands back.

use midir::MidiInputConnection;
use playback_runtime::{MidiPort, MidiSink, RuntimePlatformEffect, RuntimeStoreResult};
#[cfg(test)]
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::port_identity::{
    midi_inputs, midi_outputs, midi_ports_from_names, port_names, resolve_selected_port_id,
    stable_port_ids,
};
use super::port_watch::PortWatch;

/// Port checks (one a second) that a just-enabled or rebuilt USB MIDI gadget
/// gets to appear before its absence is logged.
pub(super) const GADGET_APPEAR_CHECKS: u8 = 10;
const PORT_CHECK_WAKE: Duration = Duration::from_secs(1);

pub(super) enum MidiConnectCommand {
    Effect(RuntimePlatformEffect),
    SetUsbMidiOutEnabled(bool),
    #[cfg(test)]
    SetTestBackend(TestMidiBackend),
    #[cfg(test)]
    Sync(std::sync::mpsc::SyncSender<()>),
}

pub(super) enum MidiConnectEvent {
    Output(Option<MidiSink>),
    Result(RuntimeStoreResult),
}

#[cfg(test)]
pub(super) struct TestMidiBackend {
    pub(super) output_names: Vec<String>,
    pub(super) input_names: Vec<String>,
    pub(super) selection_results: VecDeque<Result<(), String>>,
}

pub(super) struct MidiConnections {
    events: Sender<MidiConnectEvent>,
    midi_in: Option<MidiInputConnection<()>>,
    midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
    pub(super) host_midi_output_id: Option<String>,
    pub(super) host_midi_input_id: Option<String>,
    midi_output_error: Option<String>,
    midi_input_error: Option<String>,
    midi_output_attempted: bool,
    midi_input_attempted: bool,
    pub(super) midi_output_connected: bool,
    pub(super) midi_input_connected: bool,
    usb_midi_out_enabled: bool,
    pub(super) port_watch: PortWatch,
    pub(super) gadget_appear_checks: u8,
    #[cfg(test)]
    test_backend: Option<TestMidiBackend>,
}

impl MidiConnections {
    pub(super) fn new(
        events: Sender<MidiConnectEvent>,
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
        usb_midi_out_enabled: bool,
    ) -> Self {
        Self {
            events,
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
            gadget_appear_checks: 0,
            #[cfg(test)]
            test_backend: None,
        }
    }

    pub(super) fn run(mut self, commands: &Receiver<MidiConnectCommand>) {
        loop {
            match commands.recv_timeout(PORT_CHECK_WAKE) {
                Ok(command) => self.handle(command),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            self.reconnect_if_ports_changed(Instant::now());
        }
    }

    fn handle(&mut self, command: MidiConnectCommand) {
        match command {
            MidiConnectCommand::Effect(effect) => {
                if let Some(result) = self.handle_effect(&effect) {
                    self.emit(MidiConnectEvent::Result(result));
                }
            }
            MidiConnectCommand::SetUsbMidiOutEnabled(enabled) => {
                self.set_usb_midi_out_enabled(enabled);
            }
            #[cfg(test)]
            MidiConnectCommand::SetTestBackend(backend) => self.test_backend = Some(backend),
            #[cfg(test)]
            MidiConnectCommand::Sync(done) => {
                let _ = done.send(());
            }
        }
    }

    pub(super) fn handle_effect(
        &mut self,
        effect: &RuntimePlatformEffect,
    ) -> Option<RuntimeStoreResult> {
        match effect {
            RuntimePlatformEffect::MidiListOutputsRequest => Some(
                self.list_outputs()
                    .map(|outputs| RuntimeStoreResult::MidiListOutputsResult { outputs })
                    .unwrap_or_else(|error| list_failure(effect, error)),
            ),
            RuntimePlatformEffect::MidiListInputsRequest => Some(
                self.list_inputs()
                    .map(|inputs| RuntimeStoreResult::MidiListInputsResult { inputs })
                    .unwrap_or_else(|error| list_failure(effect, error)),
            ),
            RuntimePlatformEffect::MidiSelectOutput { id } => {
                let result = self.select_output(id.clone());
                self.status_result(result)
            }
            RuntimePlatformEffect::MidiSelectInput { id } => {
                let result = self.select_input(id.clone());
                self.status_result(result)
            }
            _ => None,
        }
    }

    fn status_result(&self, result: Result<(), String>) -> Option<RuntimeStoreResult> {
        self.selection_status(result)
            .map(|(ok, message)| RuntimeStoreResult::MidiStatus {
                ok,
                message,
                selected_out_id: self.host_midi_output_id.clone(),
                selected_in_id: self.host_midi_input_id.clone(),
            })
    }

    fn emit(&self, event: MidiConnectEvent) {
        let _ = self.events.send(event);
    }

    #[cfg(test)]
    pub(super) fn set_test_backend(&mut self, backend: TestMidiBackend) {
        self.test_backend = Some(backend);
    }

    pub(super) fn list_outputs(&self) -> Result<Vec<MidiPort>, String> {
        #[cfg(test)]
        if let Some(backend) = &self.test_backend {
            return Ok(midi_ports_from_names(&backend.output_names));
        }
        let (out, ports) = midi_outputs()?;
        Ok(midi_ports_from_names(&port_names(&out, &ports)))
    }

    pub(super) fn list_inputs(&self) -> Result<Vec<MidiPort>, String> {
        #[cfg(test)]
        if let Some(backend) = &self.test_backend {
            return Ok(midi_ports_from_names(&backend.input_names));
        }
        let (input, ports) = midi_inputs()?;
        Ok(midi_ports_from_names(&port_names(&input, &ports)))
    }

    pub(super) fn select_output(&mut self, requested: Option<String>) -> Result<(), String> {
        self.host_midi_output_id = requested.clone();
        self.midi_output_error = None;
        self.midi_output_attempted = true;
        self.midi_output_connected = false;
        if !self.usb_midi_out_enabled && requested.is_none() {
            self.emit(MidiConnectEvent::Output(None));
            return Ok(());
        }
        let result = self.connect_output(requested.as_deref());
        self.midi_output_connected = matches!(result, Ok(Some(_)));
        if matches!(result, Ok(None)) {
            log_unplugged_port("output", requested.as_deref());
        }
        match result {
            Ok(sink) => {
                self.emit(MidiConnectEvent::Output(sink));
                Ok(())
            }
            Err(error) => {
                self.emit(MidiConnectEvent::Output(None));
                self.midi_output_error = Some(error.clone());
                Err(error)
            }
        }
    }

    pub(super) fn select_input(&mut self, requested: Option<String>) -> Result<(), String> {
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

    /// Reconnects the remembered MIDI Host ports, or the USB MIDI gadget port,
    /// after the ALSA MIDI devices change: a device was plugged in or unplugged,
    /// or the gadget was rebuilt. A missing gadget port is retried on the next
    /// checks, because the gadget is torn down before it is rebuilt.
    pub(super) fn reconnect_if_ports_changed(&mut self, now: Instant) {
        match self.port_watch.check(now) {
            Some(true) => self.gadget_appear_checks = GADGET_APPEAR_CHECKS,
            Some(false) if self.gadget_appear_checks > 0 => self.gadget_appear_checks -= 1,
            _ => return,
        }
        self.reconnect();
    }

    /// Switches between the USB MIDI gadget port and the remembered MIDI Host ports.
    pub(super) fn set_usb_midi_out_enabled(&mut self, enabled: bool) {
        if self.usb_midi_out_enabled != enabled {
            self.usb_midi_out_enabled = enabled;
            self.gadget_appear_checks = GADGET_APPEAR_CHECKS;
            self.reconnect();
        }
    }

    fn reconnect(&mut self) {
        let output = self.select_output(self.host_midi_output_id.clone());
        let input = self.select_input(self.host_midi_input_id.clone());
        if !self.usb_midi_out_enabled {
            self.gadget_appear_checks = 0;
            if let Err(error) = output {
                eprintln!("MIDI output reconnect failed: {error}");
            }
            if let Err(error) = input {
                eprintln!("MIDI input reconnect failed: {error}");
            }
            return;
        }
        match output.and(input) {
            Ok(()) => {
                self.gadget_appear_checks = 0;
                eprintln!("USB MIDI gadget connected");
            }
            Err(error) if self.gadget_appear_checks == 0 => {
                eprintln!("USB MIDI gadget not connected: {error}");
            }
            Err(_) => {}
        }
    }

    pub(super) fn selection_status(
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

    fn connect_output(&mut self, requested: Option<&str>) -> Result<Option<MidiSink>, String> {
        #[cfg(test)]
        if let Some(result) = self.take_test_selection_result(requested, "output") {
            return result.map(|connected| connected.then(test_sink));
        }
        self.connect_output_native(requested)
    }

    fn connect_output_native(
        &mut self,
        requested: Option<&str>,
    ) -> Result<Option<MidiSink>, String> {
        let (out, ports) = midi_outputs()?;
        let names = port_names(&out, &ports);
        let ids = stable_port_ids(&names);
        let Some(id) =
            resolve_selected_port_id(requested, &names, &ids, self.usb_midi_out_enabled, "output")?
        else {
            return Ok(None);
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
        Ok(Some(Box::new(move |bytes| {
            connection.send(bytes).map_err(|error| error.to_string())
        })))
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

fn list_failure(effect: &RuntimePlatformEffect, error: String) -> RuntimeStoreResult {
    RuntimeStoreResult::RuntimeFailure {
        error: effect.failure_facts(error),
    }
}

#[cfg(test)]
fn test_sink() -> MidiSink {
    Box::new(|_| Ok(()))
}

fn log_unplugged_port(direction: &str, requested: Option<&str>) {
    if let Some(id) = requested {
        eprintln!("MIDI {direction} {id} is not plugged in; it connects when it appears");
    }
}

pub(super) fn usb_midi_route_error(
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
