//! Live connection changes update their System rows in place: a full menu
//! rebuild costs hundreds of milliseconds on the boards, almost all of it in
//! the Link parameter pickers.

use super::section_labels::SYSTEM_LABEL;
use super::system_bluetooth::bluetooth_group;
use super::system_io::{
    midi_host_group, midi_ports_group, sd_transfer_start_item, usb_audio_item, usb_device_group,
    MIDI_HOST_LABEL, USB_DEVICE_LABEL,
};
use super::{NativeBluetoothRows, NativeMenuItem, NativeMenuModel};

impl NativeMenuModel {
    /// Host shows the MIDI Host ports; Gadget shows USB Audio, USB MIDI and
    /// SD2 Start Transfer.
    pub fn show_usb_role_rows(
        &mut self,
        host_role: bool,
        usb_audio: bool,
        usb_midi_out_enabled: bool,
        midi_outputs: &[(String, String)],
        midi_inputs: &[(String, String)],
    ) {
        if let Some(audio) = self.system_group_containing("audioOutputs.hdmi") {
            audio
                .children
                .retain(|item| !has_key(item, "audioOutputs.usb"));
            if !host_role {
                audio.children.insert(0, usb_audio_item(usb_audio));
            }
        }
        if let Some(midi) = self.system_group_containing("midiEnabled") {
            midi.children
                .retain(|item| item.label != MIDI_HOST_LABEL && item.label != USB_DEVICE_LABEL);
            let rows = if host_role {
                midi_host_group(midi_outputs, midi_inputs)
            } else {
                usb_device_group(usb_midi_out_enabled)
            };
            midi.children.insert(1.min(midi.children.len()), rows);
        }
        if let Some(sd) = self.system_group_containing("usb.sdTransferStop") {
            sd.children
                .retain(|item| !has_key(item, "usb.sdTransferStart"));
            if !host_role {
                sd.children.insert(0, sd_transfer_start_item());
            }
        }
        self.clamp_cursor_to_siblings();
    }

    pub fn show_bluetooth_rows(&mut self, rows: NativeBluetoothRows<'_>) {
        if let Some(bluetooth) = self.system_group_containing("bluetooth.enabled") {
            *bluetooth = bluetooth_group(rows);
            self.clamp_cursor_to_siblings();
        }
    }

    pub fn show_midi_ports(&mut self, label: &str, prefix: &str, ports: &[(String, String)]) {
        if let Some(group) = self.system_group_containing(&format!("{prefix}.none")) {
            *group = midi_ports_group(label, prefix, ports);
            self.clamp_cursor_to_siblings();
        }
    }

    fn system_group_containing(&mut self, key: &str) -> Option<&mut NativeMenuItem> {
        let system = self
            .root
            .children
            .iter_mut()
            .find(|item| item.label == SYSTEM_LABEL)?;
        group_containing(system, key)
    }
}

fn has_key(item: &NativeMenuItem, key: &str) -> bool {
    item.key.as_deref() == Some(key)
}

fn group_containing<'a>(node: &'a mut NativeMenuItem, key: &str) -> Option<&'a mut NativeMenuItem> {
    if node.children.iter().any(|child| has_key(child, key)) {
        return Some(node);
    }
    node.children
        .iter_mut()
        .find_map(|child| group_containing(child, key))
}
