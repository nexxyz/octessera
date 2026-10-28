//! Incoming MIDI realtime bytes (clock, Start, Continue, Stop) become runner transport messages.

use super::{CoreRunner, HostAdapter, PlaybackRuntime, RuntimeDispatchInput, RuntimeIngest};
use crate::protocol::HostMessage;
use std::time::Instant;

impl PlaybackRuntime {
    pub fn handle_midi_realtime_bytes<R: CoreRunner, H: HostAdapter>(
        &mut self,
        bytes: &[u8],
        runner: &mut R,
        host: &mut H,
    ) -> Result<(), String> {
        let output = self.handle_midi_realtime_bytes_with_output(bytes, None, runner, host)?;
        self.requeue_discarded_output(&output);
        Ok(())
    }

    /// `received_at` is when the bytes arrived; clock-driven output is stamped from it.
    pub fn handle_midi_realtime_bytes_with_output<R: CoreRunner, H: HostAdapter>(
        &mut self,
        bytes: &[u8],
        received_at: Option<Instant>,
        runner: &mut R,
        host: &mut H,
    ) -> Result<RuntimeIngest, String> {
        self.sequenced_at = received_at.and_then(|at| at.checked_add(self.sequenced_delay));
        let output = self.dispatch_midi_realtime_bytes(bytes, runner, host);
        self.sequenced_at = None;
        output
    }

    fn dispatch_midi_realtime_bytes<R: CoreRunner, H: HostAdapter>(
        &mut self,
        bytes: &[u8],
        runner: &mut R,
        host: &mut H,
    ) -> Result<RuntimeIngest, String> {
        let mut output = RuntimeIngest::default();
        let mut clock_pulses = 0_u32;
        for byte in bytes {
            match *byte {
                0xF8 => clock_pulses = clock_pulses.saturating_add(1),
                0xFA => {
                    if clock_pulses > 0 {
                        output.merge(self.dispatch(
                            RuntimeDispatchInput::HostMessage(HostMessage::MidiRealtimeClock {
                                pulses: clock_pulses,
                            }),
                            runner,
                            host,
                        )?);
                        clock_pulses = 0;
                    }
                    output.merge(self.dispatch(
                        RuntimeDispatchInput::HostMessage(HostMessage::MidiRealtimeStart),
                        runner,
                        host,
                    )?);
                }
                0xFB => {
                    if clock_pulses > 0 {
                        output.merge(self.dispatch(
                            RuntimeDispatchInput::HostMessage(HostMessage::MidiRealtimeClock {
                                pulses: clock_pulses,
                            }),
                            runner,
                            host,
                        )?);
                        clock_pulses = 0;
                    }
                    output.merge(self.dispatch(
                        RuntimeDispatchInput::HostMessage(HostMessage::MidiRealtimeContinue),
                        runner,
                        host,
                    )?);
                }
                0xFC => {
                    if clock_pulses > 0 {
                        output.merge(self.dispatch(
                            RuntimeDispatchInput::HostMessage(HostMessage::MidiRealtimeClock {
                                pulses: clock_pulses,
                            }),
                            runner,
                            host,
                        )?);
                        clock_pulses = 0;
                    }
                    output.merge(self.dispatch(
                        RuntimeDispatchInput::HostMessage(HostMessage::MidiRealtimeStop),
                        runner,
                        host,
                    )?);
                }
                _ => {}
            }
        }

        if clock_pulses > 0 {
            output.merge(self.dispatch(
                RuntimeDispatchInput::HostMessage(HostMessage::MidiRealtimeClock {
                    pulses: clock_pulses,
                }),
                runner,
                host,
            )?);
        }
        Ok(output)
    }
}
