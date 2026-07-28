#[cfg(target_os = "linux")]
use crate::board_profiles::{OrangeEncoderPins, OrangeGpioDescriptor, ORANGE_PI_ZERO_2W_DEVICES};
#[cfg(target_os = "linux")]
use crate::encoder_gpio::{HardwareEvent, QuadratureState, SWITCH_DEBOUNCE_MS};
#[cfg(target_os = "linux")]
use gpiocdev::line::{Bias, EdgeDetection, EdgeKind, Value};
#[cfg(target_os = "linux")]
use gpiocdev::{Chip, Request};
#[cfg(target_os = "linux")]
use std::fs;
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
        if let Some(conflict) = pins.uart_conflict {
            return Err(format!(
                "{id} encoder switch on physical pin {} / GPIO offset {} conflicts with active {}",
                conflict.physical_pin, conflict.offset, conflict.signal
            ));
        }

        let chip_path = find_gpio_chip(ORANGE_PI_ZERO_2W_DEVICES.gpio, pins)?;
        let quadrature = request_quadrature(&chip_path, pins)?;
        let switch = request_switch(&chip_path, pins)?;
        let initial = quadrature_state(&quadrature, pins)?;
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let a_offset = pins.a;
        let worker = thread::Builder::new()
            .name(format!("octessera-orange-{id}"))
            .spawn(move || run_worker(id, a_offset, quadrature, switch, initial, worker_stop, tx))
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
fn quadrature_state(
    request: &Request,
    pins: &OrangeEncoderPins,
) -> Result<QuadratureState, String> {
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
    Ok(QuadratureState::new_bits((a << 1) | b))
}

#[cfg(target_os = "linux")]
fn run_worker(
    id: &'static str,
    a_offset: u32,
    quadrature: Request,
    switch: Request,
    mut state: QuadratureState,
    stop: Arc<AtomicBool>,
    tx: Sender<HardwareEvent>,
) {
    let mut quadrature_events = quadrature.edge_events();
    let mut switch_events = switch.edge_events();
    while !stop.load(Ordering::Acquire) {
        let mut handled = false;
        match quadrature_events.has_event() {
            Ok(true) => match quadrature_events.read_event() {
                Ok(event) => {
                    handled = true;
                    if let Some(delta) =
                        update_quadrature(&mut state, a_offset, event.offset, event.kind)
                    {
                        let _ = tx.send(HardwareEvent::EncoderTurn { id, delta });
                    }
                }
                Err(error) => {
                    eprintln!("{id} Orange encoder edge read failed: {error}");
                    break;
                }
            },
            Ok(false) => {}
            Err(error) => {
                eprintln!("{id} Orange encoder edge wait failed: {error}");
                break;
            }
        }
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
        if !handled {
            thread::sleep(Duration::from_millis(1));
        }
    }
}

#[cfg(target_os = "linux")]
fn update_quadrature(
    state: &mut QuadratureState,
    a_offset: u32,
    offset: u32,
    kind: EdgeKind,
) -> Option<i8> {
    let bit = edge_bit(kind);
    let next = if offset == a_offset {
        (bit << 1) | (state.bits() & 0b01)
    } else {
        (state.bits() & 0b10) | bit
    };
    state.update(next)
}

#[cfg(target_os = "linux")]
fn switch_event(id: &'static str, kind: EdgeKind) -> Option<HardwareEvent> {
    match kind {
        EdgeKind::Falling => Some(HardwareEvent::EncoderPress { id }),
        EdgeKind::Rising => Some(HardwareEvent::EncoderRelease { id }),
    }
}

#[cfg(target_os = "linux")]
fn edge_bit(kind: EdgeKind) -> u8 {
    match kind {
        EdgeKind::Rising => 1,
        EdgeKind::Falling => 0,
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
fn find_gpio_chip(plan: OrangeGpioDescriptor, pins: &OrangeEncoderPins) -> Result<PathBuf, String> {
    let required = [pins.a, pins.b, pins.sw];
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
        if info.label == plan.chip_label {
            if required.iter().any(|offset| *offset >= info.num_lines) {
                return Err(format!(
                    "GPIO chip {} has only {} lines for encoder offsets {}, {}, {}",
                    plan.chip_label, info.num_lines, pins.a, pins.b, pins.sw
                ));
            }
            return Ok(path);
        }
        candidates.push(format!("{}={}", path.display(), info.label));
    }
    Err(format!(
        "no GPIO chip label {}; candidates: {}",
        plan.chip_label,
        candidates.join(", ")
    ))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::{edge_bit, switch_event, update_quadrature, OrangeEncoderGpio};
    use crate::board_profiles::ORANGE_PI_ZERO_2W_DEVICES;
    use crate::encoder_gpio::{HardwareEvent, QuadratureState};
    use gpiocdev::line::EdgeKind;
    use std::sync::mpsc;

    #[test]
    fn quadrature_edges_preserve_the_other_channel() {
        let pins = ORANGE_PI_ZERO_2W_DEVICES.encoders[0];
        let mut state = QuadratureState::new_bits(0);
        assert_eq!(
            update_quadrature(&mut state, pins.a, pins.a, EdgeKind::Rising),
            None
        );
        assert_eq!(state.bits(), 0b10);
        assert_eq!(
            update_quadrature(&mut state, pins.a, pins.b, EdgeKind::Rising),
            None
        );
        assert_eq!(state.bits(), 0b11);
        assert_eq!(
            update_quadrature(&mut state, pins.a, pins.a, EdgeKind::Falling),
            None
        );
        assert_eq!(state.bits(), 0b01);
    }

    #[test]
    fn both_edges_map_to_the_existing_quadrature_bits() {
        assert_eq!(edge_bit(EdgeKind::Rising), 1);
        assert_eq!(edge_bit(EdgeKind::Falling), 0);
    }

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
    fn uart_conflicting_encoder_fails_before_gpio_access() {
        let (tx, _) = mpsc::channel();
        let result =
            OrangeEncoderGpio::new("encoder_aux_2", &ORANGE_PI_ZERO_2W_DEVICES.encoders[2], tx);
        let Err(error) = result else {
            panic!("UART-conflicting encoder must not request GPIO");
        };
        assert!(error.contains("UART0 TX"));
        assert!(error.contains("offset 224"));
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
}
