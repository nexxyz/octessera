use super::worker::Worker;
use super::{BluetoothHandle, Bluez, DeviceRecord, WorkerEvent};
use playback_runtime::{
    RuntimeBluetoothDeviceAction, RuntimeBluetoothDeviceKind, RuntimePlatformEffect,
};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Default)]
struct FakeBluez {
    calls: Arc<Mutex<Vec<String>>>,
    devices: Arc<Mutex<Vec<DeviceRecord>>>,
    code: Arc<Mutex<Option<String>>>,
}

impl FakeBluez {
    fn log(&self, call: impl Into<String>) {
        self.calls.lock().unwrap().push(call.into());
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl Bluez for FakeBluez {
    fn set_powered(&mut self, on: bool) -> Result<(), String> {
        self.log(format!("powered {on}"));
        Ok(())
    }
    fn powered(&mut self) -> Result<bool, String> {
        self.log("query powered");
        Ok(true)
    }
    fn set_discovering(&mut self, on: bool) -> Result<(), String> {
        self.log(format!("discovering {on}"));
        Ok(())
    }
    fn devices(&mut self) -> Result<Vec<DeviceRecord>, String> {
        Ok(self.devices.lock().unwrap().clone())
    }
    fn disconnect(&mut self, address: &str) -> Result<(), String> {
        self.log(format!("disconnect {address}"));
        Ok(())
    }
    fn forget(&mut self, address: &str) -> Result<(), String> {
        self.log(format!("forget {address}"));
        Ok(())
    }
    fn cancel_pairing(&mut self, address: &str) -> Result<(), String> {
        self.log(format!("cancel {address}"));
        Ok(())
    }
    fn spawn_pair(&mut self, address: &str, _done: Sender<WorkerEvent>) {
        self.log(format!("pair {address}"));
    }
    fn spawn_connect(&mut self, address: &str, _done: Sender<WorkerEvent>) {
        self.log(format!("connect {address}"));
    }
    fn pairing_code(&mut self) -> Option<String> {
        self.code.lock().unwrap().clone()
    }
}

fn keyboard(address: &str, name: Option<&str>, paired: bool) -> DeviceRecord {
    DeviceRecord {
        address: address.into(),
        name: name.map(Into::into),
        icon: Some("input-keyboard".into()),
        paired,
        ..DeviceRecord::default()
    }
}

fn worker_with(devices: Vec<DeviceRecord>) -> (Worker<FakeBluez>, FakeBluez) {
    let fake = FakeBluez::default();
    *fake.devices.lock().unwrap() = devices;
    let (done, _) = mpsc::channel();
    (Worker::new(fake.clone(), done), fake)
}

fn device_event(address: &str, action: RuntimeBluetoothDeviceAction) -> WorkerEvent {
    WorkerEvent::Device {
        address: address.into(),
        action,
    }
}

#[test]
fn disabled_bluetooth_publishes_nothing_and_never_calls_bluez() {
    let (mut worker, fake) = worker_with(vec![keyboard("AA", Some("Keys"), true)]);
    assert!(worker.refresh().is_none());
    assert!(fake.calls().is_empty());
}

#[test]
fn powered_bluetooth_lists_paired_devices_and_only_publishes_changes() {
    let (mut worker, fake) = worker_with(vec![
        keyboard("AA", Some("Keys"), true),
        keyboard("BB", Some("Other Keys"), false),
    ]);
    worker.handle(WorkerEvent::SetPower(true), Instant::now());
    let status = worker.refresh().expect("status");
    assert!(status.powered);
    assert_eq!(status.devices.len(), 1);
    assert_eq!(status.devices[0].address, "AA");
    assert!(worker.refresh().is_none());
    assert_eq!(fake.calls()[0], "powered true");
}

#[test]
fn scan_shows_named_keyboards_and_audio_only_and_stops_after_its_limit() {
    let mut speaker = keyboard("CC", Some("Speaker"), false);
    speaker.icon = Some("audio-card".into());
    let mut mouse = keyboard("DD", Some("Mouse"), false);
    mouse.icon = Some("input-mouse".into());
    let (mut worker, fake) = worker_with(vec![
        speaker,
        keyboard("EE", None, false),
        keyboard("BB", Some("Keys"), false),
        mouse,
    ]);
    let start = Instant::now();
    worker.handle(WorkerEvent::SetPower(true), start);
    worker.handle(WorkerEvent::Scan(true), start);
    let status = worker.refresh().expect("status");
    assert!(status.scanning);
    let found: Vec<_> = status
        .devices
        .iter()
        .map(|device| (device.address.as_str(), device.kind))
        .collect();
    assert_eq!(
        found,
        vec![
            ("BB", RuntimeBluetoothDeviceKind::Keyboard),
            ("CC", RuntimeBluetoothDeviceKind::Audio),
        ]
    );

    worker.expire_scan(start + Duration::from_secs(61));
    assert!(fake.calls().contains(&"discovering false".to_string()));
    assert!(!worker.refresh().expect("status").scanning);
}

#[test]
fn pairing_stops_the_scan_shows_the_code_and_reports_the_outcome_once() {
    let (mut worker, fake) = worker_with(vec![keyboard("BB", Some("Keys"), false)]);
    let now = Instant::now();
    worker.handle(WorkerEvent::SetPower(true), now);
    worker.handle(WorkerEvent::Scan(true), now);
    worker.refresh();
    worker.handle(device_event("BB", RuntimeBluetoothDeviceAction::Pair), now);
    assert!(fake
        .calls()
        .ends_with(&["discovering false".into(), "pair BB".into()]));

    *fake.code.lock().unwrap() = Some("123456".into());
    let pairing = worker.refresh().unwrap().pairing.expect("pairing");
    assert_eq!(pairing.name, "Keys");
    assert_eq!(pairing.code.as_deref(), Some("123456"));

    worker.handle(
        WorkerEvent::PairDone {
            address: "BB".into(),
            result: Ok(()),
        },
        now,
    );
    let status = worker.refresh().unwrap();
    assert_eq!(status.pairing, None);
    assert_eq!(status.message.as_deref(), Some("Paired Keys"));
    assert!(worker.refresh().is_none());
}

#[test]
fn cancel_ends_pairing_and_disabled_bluetooth_ignores_device_actions() {
    let (mut worker, fake) = worker_with(vec![keyboard("AA", Some("Keys"), true)]);
    let now = Instant::now();
    worker.handle(
        device_event("AA", RuntimeBluetoothDeviceAction::Forget),
        now,
    );
    assert!(fake.calls().is_empty());

    worker.handle(WorkerEvent::SetPower(true), now);
    worker.handle(device_event("AA", RuntimeBluetoothDeviceAction::Pair), now);
    worker.handle(
        device_event("AA", RuntimeBluetoothDeviceAction::CancelPairing),
        now,
    );
    assert!(fake.calls().contains(&"cancel AA".to_string()));
    assert_eq!(worker.refresh().unwrap().pairing, None);
}

#[test]
fn handle_takes_only_bluetooth_effects() {
    let handle = BluetoothHandle::with_fake(FakeBluez::default());
    assert!(handle.handle_effect(&RuntimePlatformEffect::BluetoothScan { active: true }));
    assert!(!handle.handle_effect(&RuntimePlatformEffect::MidiPanic));
}

#[test]
fn device_kind_falls_back_to_class_and_service_uuids() {
    let mut record = DeviceRecord {
        class: Some(0x0540),
        ..DeviceRecord::default()
    };
    assert_eq!(record.kind(), RuntimeBluetoothDeviceKind::Keyboard);
    record.class = Some(0x240404);
    assert_eq!(record.kind(), RuntimeBluetoothDeviceKind::Audio);
    record.class = None;
    record.uuids = vec!["0000110B-0000-1000-8000-00805F9B34FB".into()];
    assert_eq!(record.kind(), RuntimeBluetoothDeviceKind::Audio);
}
