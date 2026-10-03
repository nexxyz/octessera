use super::{status_message, Bluez, WorkerEvent};
use playback_runtime::{
    HostMessage, RuntimeBluetoothDevice, RuntimeBluetoothDeviceAction, RuntimeBluetoothDeviceKind,
    RuntimeBluetoothPairing, RuntimeBluetoothStatus,
};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

const SCAN_LIMIT: Duration = Duration::from_secs(60);
const ACTIVE_POLL: Duration = Duration::from_millis(500);
const IDLE_POLL: Duration = Duration::from_millis(1500);

pub(super) fn run(
    bluez: impl Bluez,
    events: Receiver<WorkerEvent>,
    done: Sender<WorkerEvent>,
    status: Sender<HostMessage>,
) {
    let mut worker = Worker::new(bluez, done);
    loop {
        match events.recv_timeout(worker.poll_interval()) {
            Ok(event) => worker.handle(event, Instant::now()),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        while let Ok(event) = events.try_recv() {
            worker.handle(event, Instant::now());
        }
        worker.expire_scan(Instant::now());
        if let Some(next) = worker.refresh() {
            if status.send(status_message(next)).is_err() {
                return;
            }
        }
    }
}

pub(super) struct Worker<B> {
    bluez: B,
    done: Sender<WorkerEvent>,
    enabled: bool,
    scan_until: Option<Instant>,
    pairing: Option<RuntimeBluetoothPairing>,
    message: Option<String>,
    last: RuntimeBluetoothStatus,
}

impl<B: Bluez> Worker<B> {
    pub(super) fn new(bluez: B, done: Sender<WorkerEvent>) -> Self {
        Self {
            bluez,
            done,
            enabled: false,
            scan_until: None,
            pairing: None,
            message: None,
            last: RuntimeBluetoothStatus::default(),
        }
    }

    fn poll_interval(&self) -> Duration {
        if self.scan_until.is_some() || self.pairing.is_some() {
            ACTIVE_POLL
        } else {
            IDLE_POLL
        }
    }

    pub(super) fn handle(&mut self, event: WorkerEvent, now: Instant) {
        match event {
            WorkerEvent::SetPower(on) => {
                self.enabled = on;
                if !on {
                    self.scan_until = None;
                    self.pairing = None;
                }
                let result = self.bluez.set_powered(on);
                if on {
                    self.report(result, "Bluetooth unavailable");
                }
            }
            WorkerEvent::Scan(true) if self.enabled => match self.bluez.set_discovering(true) {
                Ok(()) => self.scan_until = Some(now + SCAN_LIMIT),
                Err(_) => self.message = Some("Search failed".into()),
            },
            WorkerEvent::Scan(_) => self.stop_scan(),
            WorkerEvent::Device { address, action } if self.enabled => {
                self.device_action(&address, action);
            }
            WorkerEvent::Device { .. } => {}
            WorkerEvent::PairDone { address, result } => {
                let name = match self.pairing.take() {
                    Some(pairing) if pairing.address == address => pairing.name,
                    _ => self.name_of(&address),
                };
                self.message = Some(match result {
                    Ok(()) => format!("Paired {name}"),
                    Err(error) => {
                        eprintln!("Bluetooth pairing {address} failed: {error}");
                        format!("Pairing failed: {name}")
                    }
                });
            }
            WorkerEvent::ConnectDone { address, result } => {
                let name = self.name_of(&address);
                self.message = Some(match result {
                    Ok(()) => format!("Connected {name}"),
                    Err(error) => {
                        eprintln!("Bluetooth connect {address} failed: {error}");
                        format!("Connect failed: {name}")
                    }
                });
            }
        }
    }

    fn device_action(&mut self, address: &str, action: RuntimeBluetoothDeviceAction) {
        match action {
            RuntimeBluetoothDeviceAction::Pair if self.pairing.is_none() => {
                self.stop_scan();
                self.pairing = Some(RuntimeBluetoothPairing {
                    address: address.into(),
                    name: self.name_of(address),
                    code: None,
                });
                self.bluez.spawn_pair(address, self.done.clone());
            }
            RuntimeBluetoothDeviceAction::Pair => {}
            RuntimeBluetoothDeviceAction::CancelPairing => {
                let _ = self.bluez.cancel_pairing(address);
                self.pairing = None;
            }
            RuntimeBluetoothDeviceAction::Connect => {
                self.bluez.spawn_connect(address, self.done.clone());
            }
            RuntimeBluetoothDeviceAction::Disconnect => {
                let result = self.bluez.disconnect(address);
                self.report(result, "Disconnect failed");
            }
            RuntimeBluetoothDeviceAction::Forget => {
                let result = self.bluez.forget(address);
                self.report(result, "Forget failed");
            }
        }
    }

    fn report(&mut self, result: Result<(), String>, message: &str) {
        if let Err(error) = result {
            eprintln!("{message}: {error}");
            self.message = Some(message.into());
        }
    }

    fn stop_scan(&mut self) {
        if self.scan_until.take().is_some() {
            let _ = self.bluez.set_discovering(false);
        }
    }

    pub(super) fn expire_scan(&mut self, now: Instant) {
        if self.scan_until.is_some_and(|until| now >= until) {
            self.stop_scan();
        }
    }

    fn name_of(&self, address: &str) -> String {
        self.last
            .devices
            .iter()
            .find(|device| device.address == address)
            .map(|device| device.name.clone())
            .unwrap_or_else(|| address.into())
    }

    /// The status to publish, or `None` when nothing changed since last time.
    /// The runner starts from the empty status, so an idle radio sends nothing.
    pub(super) fn refresh(&mut self) -> Option<RuntimeBluetoothStatus> {
        let powered = self.enabled && self.bluez.powered().unwrap_or(false);
        let scanning = self.scan_until.is_some();
        let mut devices = if powered {
            self.bluez.devices().unwrap_or_default()
        } else {
            Vec::new()
        };
        devices.sort_by(|a, b| (&a.name, &a.address).cmp(&(&b.name, &b.address)));
        if let Some(pairing) = self
            .pairing
            .as_mut()
            .filter(|pairing| pairing.code.is_none())
        {
            pairing.code = self.bluez.pairing_code();
        }
        let status = RuntimeBluetoothStatus {
            powered,
            scanning,
            devices: devices
                .iter()
                .filter(|device| device.paired || (scanning && device.name.is_some()))
                .map(|device| device.to_runtime())
                .filter(|device| device.paired || device.kind != RuntimeBluetoothDeviceKind::Other)
                .collect::<Vec<RuntimeBluetoothDevice>>(),
            pairing: self.pairing.clone(),
            message: self.message.take(),
        };
        if status.message.is_none() && self.last == status {
            return None;
        }
        self.last = RuntimeBluetoothStatus {
            message: None,
            ..status.clone()
        };
        Some(status)
    }
}
