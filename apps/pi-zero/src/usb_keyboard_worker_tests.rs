#[cfg(not(target_os = "linux"))]
use super::KeyboardCapture;
use super::{worker, HostMessage, KeyboardCaptureControl, KeyboardInput, KeyboardKey};
use serde_json::Value;
use std::collections::VecDeque;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

type FakeEvents = Arc<Mutex<VecDeque<io::Result<Vec<(KeyboardKey, i32)>>>>>;

struct FakeDevice {
    id: usize,
    events: FakeEvents,
    ungrabs: Arc<Mutex<Vec<usize>>>,
}

impl FakeDevice {
    fn new(
        id: usize,
        events: impl IntoIterator<Item = io::Result<Vec<(KeyboardKey, i32)>>>,
        ungrabs: Arc<Mutex<Vec<usize>>>,
    ) -> Self {
        Self {
            id,
            events: Arc::new(Mutex::new(events.into_iter().collect())),
            ungrabs,
        }
    }

    fn with_event_queue(id: usize, ungrabs: Arc<Mutex<Vec<usize>>>) -> (Self, FakeEvents) {
        let events = Arc::new(Mutex::new(VecDeque::new()));
        (
            Self {
                id,
                events: events.clone(),
                ungrabs,
            },
            events,
        )
    }
}

impl worker::KeyboardDevice for FakeDevice {
    fn read_events(&mut self) -> io::Result<Vec<(KeyboardKey, i32)>> {
        self.events.lock().unwrap().pop_front().unwrap_or_else(|| {
            Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "fake device has no events",
            ))
        })
    }

    fn ungrab(&mut self) {
        self.ungrabs.lock().unwrap().push(self.id);
    }
}

struct FakeAcquirer {
    outcomes: VecDeque<worker::Acquisition<FakeDevice>>,
    attempts: Arc<AtomicUsize>,
}

impl worker::KeyboardAcquirer for FakeAcquirer {
    type Device = FakeDevice;

    fn discover_first(&mut self) -> worker::Acquisition<Self::Device> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        self.outcomes
            .pop_front()
            .unwrap_or(worker::Acquisition::None)
    }
}

struct WorkerHarness {
    control: KeyboardCaptureControl,
    input_rx: Option<Receiver<HostMessage>>,
    ungrabs: Arc<Mutex<Vec<usize>>>,
    attempts: Arc<AtomicUsize>,
    finished: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl WorkerHarness {
    fn enable(&self) {
        self.control
            .observe_snapshot(&serde_json::json!({ "hdmi": { "mode": "live-grid" } }));
    }

    fn disable(&self) {
        self.control
            .observe_snapshot(&serde_json::json!({ "hdmi": { "mode": "none" } }));
    }

    fn recv_input(&self) -> Value {
        match self
            .input_rx
            .as_ref()
            .unwrap()
            .recv_timeout(Duration::from_millis(100))
            .expect("worker input")
        {
            HostMessage::DeviceInput { input, .. } => input,
            message => panic!("unexpected worker message: {message:?}"),
        }
    }

    fn stop(&mut self) {
        self.control.stop();
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }

    fn ungrabbed(&self) -> Vec<usize> {
        self.ungrabs.lock().unwrap().clone()
    }
}

impl Drop for WorkerHarness {
    fn drop(&mut self) {
        self.stop();
    }
}

fn harness(
    outcomes: impl IntoIterator<Item = worker::Acquisition<FakeDevice>>,
    ungrabs: Arc<Mutex<Vec<usize>>>,
) -> WorkerHarness {
    let (input_tx, input_rx) = mpsc::channel();
    let outcomes = outcomes.into_iter().collect::<VecDeque<_>>();
    let control = KeyboardCaptureControl::new(true);
    let worker_control = control.clone();
    let attempts = Arc::new(AtomicUsize::new(0));
    let finished = Arc::new(AtomicBool::new(false));
    let worker_finished = finished.clone();
    let worker_attempts = attempts.clone();
    let worker = thread::spawn(move || {
        worker::run_with_acquirer(
            worker_control,
            input_tx,
            FakeAcquirer {
                outcomes,
                attempts: worker_attempts,
            },
        );
        worker_finished.store(true, Ordering::SeqCst);
    });
    WorkerHarness {
        control,
        input_rx: Some(input_rx),
        ungrabs,
        attempts,
        finished,
        worker: Some(worker),
    }
}

fn ready_device(
    id: usize,
    events: impl IntoIterator<Item = io::Result<Vec<(KeyboardKey, i32)>>>,
    ungrabs: &Arc<Mutex<Vec<usize>>>,
) -> worker::Acquisition<FakeDevice> {
    worker::Acquisition::Ready(FakeDevice::new(id, events, ungrabs.clone()))
}

fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_millis(200);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "worker did not reach expected state"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn worker_drives_enable_events_disable_and_ordered_releases() {
    let ungrabs = Arc::new(Mutex::new(Vec::new()));
    let mut worker = harness(
        [ready_device(
            1,
            [Ok(vec![
                (KeyboardKey::Backspace, 1),
                (KeyboardKey::Q, 1),
                (KeyboardKey::W, 1),
                (KeyboardKey::E, 1),
                (KeyboardKey::Space, 1),
            ])],
            &ungrabs,
        )],
        ungrabs.clone(),
    );
    worker.enable();
    assert_eq!(
        worker.recv_input(),
        json_input(KeyboardInput::ButtonA(true))
    );
    assert_eq!(
        worker.recv_input(),
        serde_json::json!({ "type": "encoder_turn", "delta": -1, "id": "aux1" })
    );
    assert_eq!(
        worker.recv_input(),
        serde_json::json!({ "type": "encoder_press", "id": "aux1" })
    );
    assert_eq!(
        worker.recv_input(),
        serde_json::json!({ "type": "encoder_turn", "delta": 1, "id": "aux1" })
    );
    assert_eq!(
        worker.recv_input(),
        json_input(KeyboardInput::ButtonS(true))
    );
    worker.disable();
    assert_eq!(
        worker.recv_input(),
        json_input(KeyboardInput::ButtonA(false))
    );
    assert_eq!(
        worker.recv_input(),
        json_input(KeyboardInput::ButtonS(false))
    );
    wait_until(|| worker.ungrabbed() == vec![1]);
    worker.stop();
    assert!(worker.input_rx.as_ref().unwrap().try_recv().is_err());
}

#[test]
fn worker_releases_and_ungrabs_after_read_failure() {
    let ungrabs = Arc::new(Mutex::new(Vec::new()));
    let mut worker = harness(
        [ready_device(
            2,
            [
                Ok(vec![(KeyboardKey::W, 1)]),
                Err(io::Error::new(io::ErrorKind::NotFound, "disconnected")),
            ],
            &ungrabs,
        )],
        ungrabs.clone(),
    );
    worker.enable();
    assert_eq!(
        worker.recv_input(),
        serde_json::json!({ "type": "encoder_press", "id": "aux1" })
    );
    wait_until(|| worker.ungrabbed() == vec![2]);
    assert!(worker.input_rx.as_ref().unwrap().try_recv().is_err());
    worker.stop();
}

#[test]
fn worker_aux_click_repeat_and_disable_do_not_emit_another_click() {
    let ungrabs = Arc::new(Mutex::new(Vec::new()));
    let mut worker = harness(
        [ready_device(
            8,
            [Ok(vec![
                (KeyboardKey::W, 1),
                (KeyboardKey::W, 2),
                (KeyboardKey::W, 0),
            ])],
            &ungrabs,
        )],
        ungrabs.clone(),
    );
    worker.enable();
    assert_eq!(
        worker.recv_input(),
        serde_json::json!({ "type": "encoder_press", "id": "aux1" })
    );
    thread::sleep(Duration::from_millis(10));
    worker.disable();
    wait_until(|| worker.ungrabbed() == vec![8]);
    assert!(worker.input_rx.as_ref().unwrap().try_recv().is_err());
    worker.stop();
}

#[test]
fn worker_shutdown_clears_held_aux_click_and_ungrabs() {
    let ungrabs = Arc::new(Mutex::new(Vec::new()));
    let mut worker = harness(
        [ready_device(3, [Ok(vec![(KeyboardKey::W, 1)])], &ungrabs)],
        ungrabs.clone(),
    );
    worker.enable();
    assert_eq!(
        worker.recv_input(),
        serde_json::json!({ "type": "encoder_press", "id": "aux1" })
    );
    worker.stop();
    assert!(worker.input_rx.as_ref().unwrap().try_recv().is_err());
    assert_eq!(worker.ungrabbed(), vec![3]);
}

#[test]
fn worker_exits_when_receiver_drops() {
    let ungrabs = Arc::new(Mutex::new(Vec::new()));
    let (device, events) = FakeDevice::with_event_queue(4, ungrabs.clone());
    let mut worker = harness([worker::Acquisition::Ready(device)], ungrabs.clone());
    worker.enable();
    wait_until(|| worker.attempts.load(Ordering::SeqCst) == 1);
    worker.input_rx.take();
    events
        .lock()
        .unwrap()
        .push_back(Ok(vec![(KeyboardKey::Backspace, 1)]));
    thread::sleep(Duration::from_millis(10));
    wait_until(|| worker.finished.load(Ordering::SeqCst));
    assert_eq!(worker.ungrabbed(), vec![4]);
    worker.stop();
}

#[test]
fn failed_first_grab_retries_without_selecting_a_second_device() {
    let ungrabs = Arc::new(Mutex::new(Vec::new()));
    let mut worker = harness(
        [
            worker::Acquisition::GrabFailed,
            ready_device(5, [Ok(vec![(KeyboardKey::Space, 1)])], &ungrabs),
        ],
        ungrabs.clone(),
    );
    worker.enable();
    wait_until(|| worker.attempts.load(Ordering::SeqCst) == 1);
    assert!(worker.input_rx.as_ref().unwrap().try_recv().is_err());
    worker.disable();
    thread::sleep(Duration::from_millis(10));
    worker.enable();
    assert_eq!(
        worker.recv_input(),
        json_input(KeyboardInput::ButtonS(true))
    );
    worker.stop();
    assert_eq!(
        worker.recv_input(),
        json_input(KeyboardInput::ButtonS(false))
    );
    assert_eq!(worker.attempts.load(Ordering::SeqCst), 2);
    assert_eq!(worker.ungrabbed(), vec![5]);
}

#[test]
fn selected_device_stays_owned_until_disconnect() {
    let ungrabs = Arc::new(Mutex::new(Vec::new()));
    let mut worker = harness(
        [
            ready_device(6, [Ok(vec![(KeyboardKey::Backspace, 1)])], &ungrabs),
            ready_device(7, [Ok(vec![(KeyboardKey::Space, 1)])], &ungrabs),
        ],
        ungrabs.clone(),
    );
    worker.enable();
    assert_eq!(
        worker.recv_input(),
        json_input(KeyboardInput::ButtonA(true))
    );
    thread::sleep(Duration::from_millis(25));
    assert_eq!(worker.attempts.load(Ordering::SeqCst), 1);
    assert!(worker.input_rx.as_ref().unwrap().try_recv().is_err());
    worker.disable();
    assert_eq!(
        worker.recv_input(),
        json_input(KeyboardInput::ButtonA(false))
    );
    wait_until(|| worker.ungrabbed() == vec![6]);
    worker.stop();
}

#[test]
fn keyboard_acceptance_requires_usb_bus_and_every_capability() {
    assert!(worker::accepts_keyboard(true, |_| true));
    assert!(!worker::accepts_keyboard(false, |_| true));
    assert!(!worker::accepts_keyboard(true, |key| key != KeyboardKey::Space));
    for missing in [
        KeyboardKey::Q,
        KeyboardKey::W,
        KeyboardKey::E,
        KeyboardKey::A,
        KeyboardKey::S,
        KeyboardKey::D,
        KeyboardKey::Z,
        KeyboardKey::X,
        KeyboardKey::C,
    ] {
        assert!(!worker::accepts_keyboard(true, |key| key != missing));
    }
}

#[cfg(not(target_os = "linux"))]
#[test]
fn non_linux_enabled_worker_shutdown_is_bounded() {
    let (input_tx, input_rx) = mpsc::channel();
    let capture = KeyboardCapture::spawn(input_tx, true);
    let control = capture.control();
    control.observe_snapshot(&serde_json::json!({ "hdmi": { "mode": "live-grid" } }));
    thread::sleep(Duration::from_millis(10));
    let started = Instant::now();
    drop(input_rx);
    assert!(capture.shutdown().is_ok());
    assert!(started.elapsed() < Duration::from_millis(100));
}

fn json_input(input: KeyboardInput) -> Value {
    let (input_type, pressed) = match input {
        KeyboardInput::ButtonA(pressed) => ("button_a", pressed),
        KeyboardInput::ButtonS(pressed) => ("button_s", pressed),
        KeyboardInput::ButtonShift(pressed) => ("button_shift", pressed),
        KeyboardInput::ButtonFn(pressed) => ("button_fn", pressed),
        KeyboardInput::EncoderTurn { .. } | KeyboardInput::EncoderPress { .. } => {
            panic!("test only expects button input")
        }
    };
    serde_json::json!({ "type": input_type, "pressed": pressed })
}
