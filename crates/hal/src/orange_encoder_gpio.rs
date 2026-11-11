#[cfg(target_os = "linux")]
use crate::board_profiles::{OrangeEncoderPins, ORANGE_PI_ZERO_2W_DEVICES};
#[cfg(target_os = "linux")]
use crate::encoder_gpio::{HardwareEvent, QuadratureState, SWITCH_DEBOUNCE_MS};
#[cfg(target_os = "linux")]
use gpiocdev::line::{Bias, EdgeDetection, EdgeKind, Value};
#[cfg(target_os = "linux")]
use gpiocdev::{Chip, Request};
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::os::fd::AsRawFd;
#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "linux")]
use std::sync::{mpsc::Sender, Arc};
#[cfg(target_os = "linux")]
use std::thread::{self, JoinHandle};
#[cfg(target_os = "linux")]
use std::time::Duration;

#[cfg(target_os = "linux")]
const STOP_CHECK_MS: libc::c_int = 20;

#[cfg(target_os = "linux")]
pub struct OrangeEncoderGpio {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

#[cfg(target_os = "linux")]
impl OrangeEncoderGpio {
    pub fn new(
        id: &'static str,
        pins: &OrangeEncoderPins,
        tx: Sender<HardwareEvent>,
    ) -> Result<Self, String> {
        Self::new_with_uart0_active(id, pins, true, tx)
    }

    pub fn new_with_uart0_active(
        id: &'static str,
        pins: &OrangeEncoderPins,
        uart0_active: bool,
        tx: Sender<HardwareEvent>,
    ) -> Result<Self, String> {
        let offsets: Vec<u32> = pins
            .requested_gpio_offsets(uart0_active)
            .into_iter()
            .flatten()
            .collect();
        let chip_path = find_gpio_chip(ORANGE_PI_ZERO_2W_DEVICES.gpio.chip_label, &offsets)?;
        let quadrature = request_quadrature(&chip_path, pins)?;
        let switch = pins
            .switch_available(uart0_active)
            .then(|| request_switch(&chip_path, pins))
            .transpose()?;
        let initial = QuadratureState::new_bits(quadrature_bits(&quadrature, pins)?);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let descriptor = *pins;
        let worker = thread::Builder::new()
            .name(format!("octessera-orange-{id}"))
            .spawn(move || run_worker(id, descriptor, quadrature, switch, initial, worker_stop, tx))
            .map_err(|error| format!("{id} encoder worker start failed: {error}"))?;
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
}

#[cfg(target_os = "linux")]
impl Drop for OrangeEncoderGpio {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(target_os = "linux")]
fn request_quadrature(chip_path: &Path, pins: &OrangeEncoderPins) -> Result<Request, String> {
    Request::builder()
        .on_chip(chip_path)
        .with_consumer("octessera-orange-encoder")
        .with_lines(&[pins.a, pins.b])
        .as_input()
        .with_bias(Bias::PullUp)
        .with_edge_detection(EdgeDetection::BothEdges)
        .request()
        .map_err(|error| format!("encoder A/B GPIO request failed: {error}"))
}

#[cfg(target_os = "linux")]
fn request_switch(chip_path: &Path, pins: &OrangeEncoderPins) -> Result<Request, String> {
    Request::builder()
        .on_chip(chip_path)
        .with_consumer("octessera-orange-encoder")
        .with_line(pins.sw)
        .as_input()
        .with_bias(Bias::PullUp)
        .with_edge_detection(EdgeDetection::BothEdges)
        .with_debounce_period(Duration::from_millis(SWITCH_DEBOUNCE_MS))
        .request()
        .map_err(|error| format!("encoder switch GPIO request failed: {error}"))
}

#[cfg(target_os = "linux")]
fn quadrature_bits(request: &Request, pins: &OrangeEncoderPins) -> Result<u8, String> {
    let a = value_bit(
        request
            .value(pins.a)
            .map_err(|error| format!("encoder A GPIO read failed: {error}"))?,
    );
    let b = value_bit(
        request
            .value(pins.b)
            .map_err(|error| format!("encoder B GPIO read failed: {error}"))?,
    );
    Ok((a << 1) | b)
}

#[cfg(target_os = "linux")]
fn run_worker(
    id: &'static str,
    descriptor: OrangeEncoderPins,
    quadrature: Request,
    switch: Option<Request>,
    mut state: QuadratureState,
    stop: Arc<AtomicBool>,
    tx: Sender<HardwareEvent>,
) {
    let mut quadrature_events = quadrature.edge_events();
    let mut switch_events = switch.as_ref().map(|request| request.edge_events());
    while !stop.load(Ordering::Acquire) {
        let mut handled = false;
        match quadrature_events.has_event() {
            Ok(true) => {
                if let Err(error) = quadrature_events.read_event() {
                    eprintln!("{id} Orange encoder edge read failed: {error}");
                    break;
                }
                handled = true;
                let next = match quadrature_bits(&quadrature, &descriptor) {
                    Ok(next) => next,
                    Err(error) => {
                        eprintln!("{id} Orange {error}");
                        break;
                    }
                };
                if let Some(delta) = state.update(next) {
                    let _ = tx.send(HardwareEvent::EncoderTurn {
                        id,
                        delta: descriptor.canonical_turn_delta(delta),
                    });
                }
            }
            Ok(false) => {}
            Err(error) => {
                eprintln!("{id} Orange encoder edge wait failed: {error}");
                break;
            }
        }
        if let Some(switch_events) = switch_events.as_mut() {
            match switch_events.has_event() {
                Ok(true) => match switch_events.read_event() {
                    Ok(event) => {
                        handled = true;
                        if let Some(message) = switch_event(id, event.kind) {
                            let _ = tx.send(message);
                        }
                    }
                    Err(error) => {
                        eprintln!("{id} Orange encoder switch read failed: {error}");
                        break;
                    }
                },
                Ok(false) => {}
                Err(error) => {
                    eprintln!("{id} Orange encoder switch wait failed: {error}");
                    break;
                }
            }
        }
        if !handled {
            wait_for_edges(&quadrature, switch.as_ref());
        }
    }
}

/// Sleeps until either request has an edge queued, waking every `STOP_CHECK_MS` so a dropped
/// encoder's worker notices the stop flag.
#[cfg(target_os = "linux")]
fn wait_for_edges(quadrature: &Request, switch: Option<&Request>) {
    let poll_fd = |request: &Request| libc::pollfd {
        fd: request.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let mut fds = [poll_fd(quadrature), poll_fd(switch.unwrap_or(quadrature))];
    let count = if switch.is_some() { 2 } else { 1 };
    unsafe { libc::poll(fds.as_mut_ptr(), count, STOP_CHECK_MS) };
}

#[cfg(target_os = "linux")]
fn switch_event(id: &'static str, kind: EdgeKind) -> Option<HardwareEvent> {
    match kind {
        EdgeKind::Falling => Some(HardwareEvent::EncoderPress { id }),
        EdgeKind::Rising => Some(HardwareEvent::EncoderRelease { id }),
    }
}

#[cfg(target_os = "linux")]
fn value_bit(value: Value) -> u8 {
    match value {
        Value::Active => 1,
        Value::Inactive => 0,
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn find_gpio_chip(chip_label: &str, offsets: &[u32]) -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    for entry in fs::read_dir("/dev").map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry.file_name().to_string_lossy().starts_with("gpiochip") {
            continue;
        }
        let path = entry.path();
        let chip = Chip::from_path(&path)
            .map_err(|error| format!("cannot open GPIO chip {}: {error}", path.display()))?;
        let info = chip
            .info()
            .map_err(|error| format!("cannot read GPIO chip {}: {error}", path.display()))?;
        if info.label == chip_label {
            if offsets.iter().any(|offset| *offset >= info.num_lines) {
                return Err(format!(
                    "GPIO chip {chip_label} has only {} lines for offsets {offsets:?}",
                    info.num_lines
                ));
            }
            return Ok(path);
        }
        candidates.push(format!("{}={}", path.display(), info.label));
    }
    Err(format!(
        "no GPIO chip label {chip_label}; candidates: {}",
        candidates.join(", ")
    ))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::switch_event;
    use crate::board_profiles::ORANGE_PI_ZERO_2W_DEVICES;
    use crate::encoder_gpio::HardwareEvent;
    use gpiocdev::line::EdgeKind;

    #[test]
    fn switch_edges_keep_active_low_hardware_event_semantics() {
        assert!(matches!(
            switch_event("encoder_main", EdgeKind::Falling),
            Some(HardwareEvent::EncoderPress { id: "encoder_main" })
        ));
        assert!(matches!(
            switch_event("encoder_main", EdgeKind::Rising),
            Some(HardwareEvent::EncoderRelease { id: "encoder_main" })
        ));
    }

    #[test]
    fn active_uart_profile_never_requests_aux2_switch_line() {
        let pins = ORANGE_PI_ZERO_2W_DEVICES.encoders[2];
        assert_eq!(
            pins.requested_gpio_offsets(true),
            [Some(227), Some(269), None]
        );
    }

    #[test]
    fn disabled_uart_profile_requests_aux2_switch_line() {
        let pins = ORANGE_PI_ZERO_2W_DEVICES.encoders[2];
        assert_eq!(
            pins.requested_gpio_offsets(false),
            [Some(227), Some(269), Some(224)]
        );
    }

    #[test]
    fn orange_event_boundary_reverses_each_board_turn() {
        let pins = ORANGE_PI_ZERO_2W_DEVICES.encoders;
        assert_eq!(
            pins.map(|value| value.canonical_turn_delta(1)),
            [-1, -1, -1, -1]
        );
        assert_eq!(
            pins.map(|value| value.canonical_turn_delta(-1)),
            [1, 1, 1, 1]
        );
    }
}

#[cfg(not(target_os = "linux"))]
pub struct OrangeEncoderGpio {
    _private: (),
}

#[cfg(not(target_os = "linux"))]
impl OrangeEncoderGpio {
    pub fn new(
        _id: &'static str,
        _pins: &crate::board_profiles::OrangeEncoderPins,
        _tx: std::sync::mpsc::Sender<crate::encoder_gpio::HardwareEvent>,
    ) -> Result<Self, String> {
        Err("Orange encoder GPIO requires a Linux target".into())
    }

    pub fn new_with_uart0_active(
        _id: &'static str,
        _pins: &crate::board_profiles::OrangeEncoderPins,
        _uart0_active: bool,
        _tx: std::sync::mpsc::Sender<crate::encoder_gpio::HardwareEvent>,
    ) -> Result<Self, String> {
        Err("Orange encoder GPIO requires a Linux target".into())
    }
}
