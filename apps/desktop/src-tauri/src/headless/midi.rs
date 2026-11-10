//! MIDI through the headless session: what the runtime sent out, and input dispatched the way
//! the desktop worker dispatches a MIDI port's messages.
use super::HeadlessSession;
use playback_runtime::{
    HostMessage, MidiControlChange, MidiPort, RuntimeDispatchInput, RuntimeStoreResult,
};

/// A MIDI out message with the transport pulse and audio frame it was sent at.
#[derive(Clone, Debug)]
pub struct PlayedMidi {
    pub pulse: u64,
    pub frame: usize,
    pub bytes: Vec<u8>,
}

impl HeadlessSession {
    /// Answers as the platform service does after listing one MIDI output and one input, so the
    /// MIDI Host menus can pick them. The port id names no real port: a pick connects nothing,
    /// and the runtime still sends to its MIDI out, which [`Self::midi_out`] records. Opening a
    /// port list scans this computer's ports, which can take longer than a quiet settle, so this
    /// first waits for that scan's answer; otherwise it would replace the test port afterwards.
    pub fn list_test_midi_ports(&mut self) -> Result<(), String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while !std::mem::take(&mut self.midi_port_scan_answered) {
            if std::time::Instant::now() >= deadline {
                return Err("the MIDI port scan did not answer within 20 s".into());
            }
            self.poll_services()?;
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let ports = vec![MidiPort {
            id: "999".into(),
            name: "e2e port".into(),
        }];
        self.dispatch(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::MidiListOutputsResult {
                outputs: ports.clone(),
            },
        })?;
        self.dispatch(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::MidiListInputsResult { inputs: ports },
        })
    }

    pub fn midi_out(&self) -> &[PlayedMidi] {
        &self.midi_out
    }

    /// One message from a MIDI input port: Control Changes as host messages, everything else
    /// through the realtime path.
    pub fn midi_in(&mut self, bytes: &[u8]) -> Result<(), String> {
        let output = match MidiControlChange::parse(bytes) {
            Some(change) => self.playback.dispatch(
                RuntimeDispatchInput::HostMessage(HostMessage::from(change)),
                &mut self.runner,
                &mut self.adapter,
            )?,
            None => self.playback.handle_midi_realtime_bytes_with_output(
                bytes,
                Some(std::time::Instant::now()),
                &mut self.runner,
                &mut self.adapter,
            )?,
        };
        self.absorb(output)?;
        self.poll_services()?;
        Ok(())
    }
}
