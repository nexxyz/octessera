use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeBluetoothDeviceAction {
    Pair,
    CancelPairing,
    Connect,
    Disconnect,
    Forget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeBluetoothDeviceKind {
    Keyboard,
    Audio,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeBluetoothDevice {
    pub address: String,
    pub name: String,
    pub kind: RuntimeBluetoothDeviceKind,
    pub paired: bool,
    pub connected: bool,
}

/// A pairing in progress. `code` is the passkey or PIN the user types on the
/// keyboard, once BlueZ asks for one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeBluetoothPairing {
    pub address: String,
    pub name: String,
    #[serde(default)]
    pub code: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeBluetoothStatus {
    pub powered: bool,
    pub scanning: bool,
    pub devices: Vec<RuntimeBluetoothDevice>,
    #[serde(default)]
    pub pairing: Option<RuntimeBluetoothPairing>,
    /// One-off outcome to show the user, such as a finished or failed pairing.
    #[serde(default)]
    pub message: Option<String>,
}
