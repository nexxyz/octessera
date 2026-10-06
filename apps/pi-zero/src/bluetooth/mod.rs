//! Bluetooth keyboards and speakers through BlueZ. One low-priority worker owns
//! the adapter; the runtime only sends commands and receives status updates.

#[cfg(target_os = "linux")]
mod agent;
#[cfg(target_os = "linux")]
mod bluez;
mod device;
mod monitor;
mod worker;

pub(crate) use device::DeviceRecord;
pub(crate) use monitor::BluetoothAudioMonitor;
use playback_runtime::{
    HostMessage, RuntimeBluetoothDeviceAction, RuntimePlatformEffect, RuntimeStoreResult,
};
use std::sync::mpsc::{Receiver, Sender};

/// Long BlueZ calls (pairing, connecting) run on short helper threads and
/// report back through the worker's event channel.
pub(crate) trait Bluez: Send + 'static {
    fn set_powered(&mut self, on: bool) -> Result<(), String>;
    fn powered(&mut self) -> Result<bool, String>;
    fn set_discovering(&mut self, on: bool) -> Result<(), String>;
    fn devices(&mut self) -> Result<Vec<DeviceRecord>, String>;
    fn disconnect(&mut self, address: &str) -> Result<(), String>;
    fn forget(&mut self, address: &str) -> Result<(), String>;
    fn cancel_pairing(&mut self, address: &str) -> Result<(), String>;
    fn spawn_pair(&mut self, address: &str, done: Sender<WorkerEvent>);
    fn spawn_connect(&mut self, address: &str, done: Sender<WorkerEvent>);
    fn pairing_code(&mut self) -> Option<String>;
}

pub(crate) enum WorkerEvent {
    SetPower(bool),
    Scan(bool),
    Device {
        address: String,
        action: RuntimeBluetoothDeviceAction,
    },
    PairDone {
        address: String,
        result: Result<(), String>,
    },
    ConnectDone {
        address: String,
        result: Result<(), String>,
    },
}

pub(crate) struct BluetoothHandle {
    events: Sender<WorkerEvent>,
    status: Receiver<HostMessage>,
}

impl BluetoothHandle {
    /// Starts the worker against the system BlueZ. Without BlueZ (non-Linux
    /// builds) every request reports that Bluetooth is unavailable.
    pub(crate) fn spawn_system() -> Option<Self> {
        #[cfg(target_os = "linux")]
        let connect = bluez::SystemBluez::connect;
        #[cfg(not(target_os = "linux"))]
        let connect = || Ok(Unavailable);
        Self::spawn(connect)
    }

    #[cfg(test)]
    pub(crate) fn with_fake(bluez: impl Bluez) -> Self {
        Self::spawn(move || Ok(bluez)).expect("spawn Bluetooth worker")
    }

    fn spawn<B: Bluez>(
        connect: impl FnOnce() -> Result<B, String> + Send + 'static,
    ) -> Option<Self> {
        let (events, rx) = std::sync::mpsc::channel();
        let (status_tx, status) = std::sync::mpsc::channel();
        let worker_events = events.clone();
        std::thread::Builder::new()
            .name("octessera-bluetooth".into())
            .spawn(move || match connect() {
                Ok(bluez) => worker::run(bluez, rx, worker_events, status_tx),
                Err(error) => eprintln!("Bluetooth unavailable: {error}"),
            })
            .ok()?;
        Some(Self { events, status })
    }

    pub(crate) fn set_enabled(&self, enabled: bool) {
        let _ = self.events.send(WorkerEvent::SetPower(enabled));
    }

    /// Takes the Bluetooth effects; any other effect is left to the caller.
    pub(crate) fn handle_effect(&self, effect: &RuntimePlatformEffect) -> bool {
        let event = match effect {
            RuntimePlatformEffect::BluetoothScan { active } => WorkerEvent::Scan(*active),
            RuntimePlatformEffect::BluetoothDevice { address, action } => WorkerEvent::Device {
                address: address.clone(),
                action: *action,
            },
            _ => return false,
        };
        let _ = self.events.send(event);
        true
    }

    pub(crate) fn drain_status(&self) -> impl Iterator<Item = HostMessage> + '_ {
        self.status.try_iter()
    }
}

pub(crate) fn status_message(status: playback_runtime::RuntimeBluetoothStatus) -> HostMessage {
    HostMessage::RuntimeResult {
        result: RuntimeStoreResult::BluetoothStatus { status },
    }
}

#[cfg(not(target_os = "linux"))]
struct Unavailable;

#[cfg(not(target_os = "linux"))]
impl Bluez for Unavailable {
    fn set_powered(&mut self, _on: bool) -> Result<(), String> {
        Err("no BlueZ".into())
    }
    fn powered(&mut self) -> Result<bool, String> {
        Ok(false)
    }
    fn set_discovering(&mut self, _on: bool) -> Result<(), String> {
        Err("no BlueZ".into())
    }
    fn devices(&mut self) -> Result<Vec<DeviceRecord>, String> {
        Ok(Vec::new())
    }
    fn disconnect(&mut self, _address: &str) -> Result<(), String> {
        Err("no BlueZ".into())
    }
    fn forget(&mut self, _address: &str) -> Result<(), String> {
        Err("no BlueZ".into())
    }
    fn cancel_pairing(&mut self, _address: &str) -> Result<(), String> {
        Ok(())
    }
    fn spawn_pair(&mut self, address: &str, done: Sender<WorkerEvent>) {
        let _ = done.send(WorkerEvent::PairDone {
            address: address.into(),
            result: Err("no BlueZ".into()),
        });
    }
    fn spawn_connect(&mut self, address: &str, done: Sender<WorkerEvent>) {
        let _ = done.send(WorkerEvent::ConnectDone {
            address: address.into(),
            result: Err("no BlueZ".into()),
        });
    }
    fn pairing_code(&mut self) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests;
