use playback_runtime::{
    HostAdapter, HostMessage, NativeRunner, PlaybackRuntime, RuntimePlatformEffect, TimedMidiSender,
};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use crate::input::MidiMessage;
use connections::{MidiConnectCommand, MidiConnectEvent, MidiConnections};

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

/// The runtime side of MIDI: sends go to the timed sender thread, and port
/// listing and connecting go to the connection worker, so neither blocks the
/// runtime thread.
pub(crate) struct MidiHost {
    midi_out: TimedMidiSender,
    commands: Sender<MidiConnectCommand>,
    events: Receiver<MidiConnectEvent>,
    host_midi_output_id: Option<String>,
    host_midi_input_id: Option<String>,
}

impl MidiHost {
    pub(crate) fn new(
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
        usb_midi_out_enabled: bool,
    ) -> Self {
        let (commands, command_rx) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        let connections = MidiConnections::new(event_tx, midi_in_handler, usb_midi_out_enabled);
        std::thread::Builder::new()
            .name("octessera-midi-connect".into())
            .spawn(move || connections.run(&command_rx))
            .expect("MIDI connection worker should start");
        Self {
            midi_out: TimedMidiSender::new("octessera-midi-out", midi_sender_start_hook),
            commands,
            events,
            host_midi_output_id: None,
            host_midi_input_id: None,
        }
    }

    /// Hands a MIDI list or select request to the connection worker; its
    /// result arrives through [`Self::drain_results`].
    pub(crate) fn request(&mut self, effect: &RuntimePlatformEffect) -> bool {
        match effect {
            RuntimePlatformEffect::MidiSelectOutput { id } => {
                self.host_midi_output_id = id.clone();
            }
            RuntimePlatformEffect::MidiSelectInput { id } => {
                self.host_midi_input_id = id.clone();
            }
            RuntimePlatformEffect::MidiListOutputsRequest
            | RuntimePlatformEffect::MidiListInputsRequest => {}
            _ => return false,
        }
        self.command(MidiConnectCommand::Effect(effect.clone()));
        true
    }

    /// Switches between the USB MIDI gadget port and the remembered MIDI Host ports.
    pub(crate) fn set_usb_midi_out_enabled(&self, enabled: bool) {
        self.command(MidiConnectCommand::SetUsbMidiOutEnabled(enabled));
    }

    /// Installs the output the worker connected and returns its results for the runner.
    pub(crate) fn drain_results(&mut self) -> Vec<HostMessage> {
        let mut results = Vec::new();
        while let Ok(event) = self.events.try_recv() {
            match event {
                MidiConnectEvent::Output(sink) => {
                    if let Err(error) = self.midi_out.set_sink(sink) {
                        eprintln!("MIDI output switch failed: {error}");
                    }
                }
                MidiConnectEvent::Result(result) => {
                    results.push(HostMessage::RuntimeResult { result });
                }
            }
        }
        results
    }

    fn command(&self, command: MidiConnectCommand) {
        if self.commands.send(command).is_err() {
            eprintln!("MIDI connection worker stopped");
        }
    }

    pub(crate) fn send(
        &mut self,
        at: Option<std::time::Instant>,
        bytes: &[u8],
    ) -> Result<(), String> {
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

    #[cfg(test)]
    pub(crate) fn set_test_backend(
        &mut self,
        output_names: impl IntoIterator<Item = String>,
        input_names: impl IntoIterator<Item = String>,
        results: impl IntoIterator<Item = Result<(), String>>,
    ) {
        self.command(MidiConnectCommand::SetTestBackend(
            test_backend::TestMidiBackend {
                output_names: output_names.into_iter().collect(),
                input_names: input_names.into_iter().collect(),
                selection_results: results.into_iter().collect(),
            },
        ));
    }

    /// Waits until the worker has handled every earlier request.
    #[cfg(test)]
    pub(crate) fn sync_worker(&self) {
        let (done, finished) = mpsc::sync_channel(1);
        self.command(MidiConnectCommand::Sync(done));
        finished
            .recv()
            .expect("MIDI connection worker should answer");
    }
}

fn midi_sender_start_hook() {
    #[cfg(any(
        feature = "hardware-raspberry-pi-zero-2w",
        feature = "hardware-orange-pi-zero-2w"
    ))]
    crate::audio_priority::pin_midi_sender_thread();
}

mod connections;
mod port_identity;
mod port_watch;
#[cfg(test)]
mod startup_tests;
#[cfg(test)]
mod test_backend;
#[cfg(test)]
mod tests;
