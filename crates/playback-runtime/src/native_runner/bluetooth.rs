use super::snapshot_display::DisplaySnapshot;
use super::{DeviceInput, NativeRunner, OLED_BODY_ROWS};
use crate::oled_frame::OledDisplayLayout;
use crate::protocol::{
    RunnerMessage, RuntimeBluetoothDevice, RuntimeBluetoothDeviceAction,
    RuntimeBluetoothDeviceKind, RuntimeBluetoothPairing, RuntimeBluetoothStatus,
    RuntimePlatformEffect, RuntimeStoreResult,
};

pub(super) const PAIR_NEW_LABEL: &str = "Pair New";
const PAIR_NEW_PATH: &str = "Bluetooth > Pair New";

#[derive(Clone, Debug, Default)]
pub(super) struct NativeBluetoothState {
    pub(super) enabled: bool,
    pub(super) status: Option<RuntimeBluetoothStatus>,
}

impl NativeRunner {
    pub fn bluetooth_enabled(&self) -> bool {
        self.bluetooth.enabled
    }
}

impl NativeBluetoothState {
    fn connected(&self, address: &str) -> bool {
        self.status.as_ref().is_some_and(|status| {
            status
                .devices
                .iter()
                .any(|device| device.address == address && device.connected)
        })
    }

    pub(super) fn pairing(&self) -> Option<&RuntimeBluetoothPairing> {
        self.status.as_ref()?.pairing.as_ref()
    }

    /// Paired devices as (address, label) menu rows.
    pub(super) fn paired_rows(&self) -> Vec<(String, String)> {
        self.devices()
            .filter(|device| device.paired)
            .map(|device| {
                let state = if device.connected { " - on" } else { "" };
                (
                    device.address.clone(),
                    format!("{}{state}", device_label(device)),
                )
            })
            .collect()
    }

    /// Unpaired keyboards found by a scan, as menu rows. Speakers wait for
    /// Bluetooth audio output.
    pub(super) fn found_rows(&self) -> Vec<(String, String)> {
        self.devices()
            .filter(|device| !device.paired && device.kind == RuntimeBluetoothDeviceKind::Keyboard)
            .map(|device| (device.address.clone(), device_label(device)))
            .collect()
    }

    fn devices(&self) -> impl Iterator<Item = &RuntimeBluetoothDevice> {
        self.status.iter().flat_map(|status| status.devices.iter())
    }
}

fn device_label(device: &RuntimeBluetoothDevice) -> String {
    format!("{} [{}]", device.name, kind_label(device.kind))
}

fn kind_label(kind: RuntimeBluetoothDeviceKind) -> &'static str {
    match kind {
        RuntimeBluetoothDeviceKind::Keyboard => "kbd",
        RuntimeBluetoothDeviceKind::Audio => "audio",
        RuntimeBluetoothDeviceKind::Other => "other",
    }
}

impl NativeRunner {
    pub(super) fn apply_bluetooth_payload(&mut self, runtime: &serde_json::Value) {
        if let Some(enabled) = runtime
            .get("bluetooth")
            .and_then(|bluetooth| bluetooth.get("enabled"))
            .and_then(serde_json::Value::as_bool)
        {
            self.bluetooth.enabled = enabled;
        }
    }

    #[cfg(test)]
    pub(super) fn apply_bluetooth_menu_state(&mut self) -> bool {
        let Some(enabled) = self.menu.value_for_key("bluetooth.enabled") else {
            return false;
        };
        let enabled = enabled == "true";
        let changed = self.bluetooth.enabled != enabled;
        self.bluetooth.enabled = enabled;
        changed
    }

    pub(super) fn apply_bluetooth_result(&mut self, result: RuntimeStoreResult) {
        let RuntimeStoreResult::BluetoothStatus { mut status } = result else {
            return;
        };
        let message = status.message.take();
        self.bluetooth.status = Some(status);
        self.menu.rebuild(self.menu_config());
        if let Some(message) = message {
            self.show_toast(message);
        }
    }

    pub(super) fn bluetooth_effect_for_action(
        &self,
        action: &str,
    ) -> Option<RuntimePlatformEffect> {
        let (verb, address) = action.strip_prefix("bluetooth.")?.split_once(':')?;
        let action = match verb {
            "pair" => RuntimeBluetoothDeviceAction::Pair,
            "forget" => RuntimeBluetoothDeviceAction::Forget,
            "toggle" if self.bluetooth.connected(address) => {
                RuntimeBluetoothDeviceAction::Disconnect
            }
            "toggle" => RuntimeBluetoothDeviceAction::Connect,
            _ => return None,
        };
        Some(RuntimePlatformEffect::BluetoothDevice {
            address: address.to_string(),
            action,
        })
    }

    pub(super) fn in_bluetooth_pair_page(&self) -> bool {
        self.menu.current_group_path().ends_with(PAIR_NEW_PATH)
    }

    pub(super) fn handle_bluetooth_pairing_input(
        &mut self,
        input: DeviceInput,
    ) -> Result<Vec<RunnerMessage>, String> {
        let cancel = matches!(
            input,
            DeviceInput::EncoderPress { ref id } if id.as_deref().unwrap_or("main") == "main"
        ) || matches!(input, DeviceInput::ButtonA { pressed } if pressed.unwrap_or(true));
        let Some(pairing) = self.bluetooth.pairing().filter(|_| cancel) else {
            return self.messages_with_snapshot();
        };
        let effect = RuntimePlatformEffect::BluetoothDevice {
            address: pairing.address.clone(),
            action: RuntimeBluetoothDeviceAction::CancelPairing,
        };
        if let Some(status) = self.bluetooth.status.as_mut() {
            status.pairing = None;
        }
        self.messages_with_effects(vec![effect])
    }
}

pub(super) fn bluetooth_pairing_display(pairing: &RuntimeBluetoothPairing) -> DisplaySnapshot {
    let mut lines = vec![pairing.name.clone()];
    match &pairing.code {
        Some(code) => {
            lines.push("Type on keyboard:".into());
            lines.push(format!("{code} + Enter"));
        }
        None => lines.push("Pairing...".into()),
    }
    lines.push("> Cancel".into());
    lines.truncate(OLED_BODY_ROWS);
    let line_count = lines.len();
    DisplaySnapshot {
        body_layout: OledDisplayLayout::Rows,
        title: "Bluetooth".into(),
        lines,
        colors: vec![platform_core::palette::WHITE_RGB565; line_count],
        bar_values: vec![None; line_count],
        full_lines: vec![None; line_count],
        scroll: None,
        selected_row: Some(line_count.saturating_sub(1)),
    }
}
