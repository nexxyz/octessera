//! Reconnecting the selected MIDI ports when the ALSA MIDI devices change or
//! USB MIDI switches between the gadget and the MIDI Host ports.

use super::{MidiConnectEvent, MidiConnections, GADGET_APPEAR_CHECKS};
use crate::midi_host::port_identity::stable_port_ids;
use crate::midi_host::port_watch::NodeChange;
use std::time::Instant;

impl MidiConnections {
    /// Reconnects the remembered MIDI Host ports, or the USB MIDI gadget port,
    /// after the ALSA MIDI devices change: a device was plugged in or unplugged,
    /// or the gadget was rebuilt. A connected MIDI Host port whose device is
    /// still listed stays connected when another device comes or goes, since
    /// replacing the output sink flushes its pending note-offs. A missing
    /// gadget direction is retried on the next checks, because the gadget is
    /// torn down before it is rebuilt; a retry keeps the connected direction.
    pub(in crate::midi_host) fn reconnect_if_ports_changed(&mut self, now: Instant) {
        match self.port_watch.check(now) {
            Some(NodeChange::Recreated) => {
                self.gadget_appear_checks = GADGET_APPEAR_CHECKS;
                self.reconnect(true, true);
            }
            Some(NodeChange::AddedOrRemoved) => {
                self.gadget_appear_checks = GADGET_APPEAR_CHECKS;
                let output = self.port_needs_reconnect(
                    self.midi_output_connected,
                    &self.host_midi_output_id,
                    || self.output_port_names(),
                );
                let input = self.port_needs_reconnect(
                    self.midi_input_connected,
                    &self.host_midi_input_id,
                    || self.input_port_names(),
                );
                if output || input {
                    self.reconnect(output, input);
                } else {
                    self.gadget_appear_checks = 0;
                }
            }
            Some(NodeChange::Unchanged) if self.gadget_appear_checks > 0 => {
                self.gadget_appear_checks -= 1;
                self.reconnect(!self.midi_output_connected, !self.midi_input_connected);
            }
            _ => {}
        }
    }

    fn port_needs_reconnect(
        &self,
        connected: bool,
        id: &Option<String>,
        names: impl FnOnce() -> Result<Vec<String>, String>,
    ) -> bool {
        if self.usb_midi_out_enabled {
            return true;
        }
        let Some(id) = id else {
            return false;
        };
        !connected || !names().is_ok_and(|names| stable_port_ids(&names).contains(id))
    }

    /// Switches between the USB MIDI gadget port and the remembered MIDI Host ports.
    pub(in crate::midi_host) fn set_usb_midi_out_enabled(&mut self, enabled: bool) {
        if self.usb_midi_out_enabled != enabled {
            self.usb_midi_out_enabled = enabled;
            self.gadget_appear_checks = GADGET_APPEAR_CHECKS;
            self.reconnect(true, true);
        }
    }

    fn reconnect(&mut self, output: bool, input: bool) {
        let output = if output {
            self.select_output(self.host_midi_output_id.clone())
        } else {
            Ok(())
        };
        let input = if input {
            self.select_input(self.host_midi_input_id.clone())
        } else {
            Ok(())
        };
        if !self.usb_midi_out_enabled {
            self.gadget_appear_checks = 0;
            match (output, input) {
                (Ok(()), Ok(())) => {
                    if let Some(status) = self.status_result(Ok(())) {
                        self.emit(MidiConnectEvent::Result(status));
                    }
                }
                (output, input) => {
                    if let Err(error) = output {
                        eprintln!("MIDI output reconnect failed: {error}");
                    }
                    if let Err(error) = input {
                        eprintln!("MIDI input reconnect failed: {error}");
                    }
                }
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
}
