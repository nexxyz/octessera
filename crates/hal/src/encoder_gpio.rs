//! Rotary encoder input via GPIO (quadrature + push switch)
//! Uses rppal for interrupt-driven decoding on Pi Zero 2W

#[cfg(feature = "pi-zero")]
use rppal::gpio::{Event, Gpio, InputPin, Level, Trigger};
use std::sync::mpsc::Sender;
#[cfg(feature = "pi-zero")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "pi-zero")]
use std::time::{Duration, Instant};

#[cfg(not(feature = "pi-zero"))]
use std::fmt;

/// Hardware event from encoders
#[derive(Debug, Clone, Copy)]
pub enum HardwareEvent {
    EncoderTurn { id: &'static str, delta: i8 },
    EncoderPress { id: &'static str },
}

#[cfg(feature = "pi-zero")]
const SWITCH_DEBOUNCE_MS: u64 = 45;

/// Rotary encoder with GPIO interrupt handling
#[cfg(feature = "pi-zero")]
pub struct EncoderGpio {
    _id: &'static str,
    _a: InputPin,
    _b: InputPin,
    _sw: InputPin,
    _state: Arc<Mutex<QuadratureState>>,
    _tx: Sender<HardwareEvent>,
}

#[cfg(feature = "pi-zero")]
struct QuadratureState {
    last: u8,
    accum: i8,
}

#[cfg(feature = "pi-zero")]
impl QuadratureState {
    fn new(a: Level, b: Level) -> Self {
        Self {
            last: levels_to_bits(a, b),
            accum: 0,
        }
    }

    fn update(&mut self, next: u8) -> Option<i8> {
        let transition = (self.last << 2) | next;
        self.last = next;
        let step = match transition {
            0b0001 | 0b0111 | 0b1110 | 0b1000 => 1,
            0b0010 | 0b1011 | 0b1101 | 0b0100 => -1,
            0b0011 | 0b1100 => {
                self.accum = 0;
                return None;
            }
            _ => 0,
        };
        self.accum += step;
        if self.accum >= 4 {
            self.accum = 0;
            Some(1)
        } else if self.accum <= -4 {
            self.accum = 0;
            Some(-1)
        } else {
            None
        }
    }
}

#[cfg(feature = "pi-zero")]
impl EncoderGpio {
    /// Create new encoder on given pins (A, B, Switch)
    pub fn new(
        id: &'static str,
        pins: &crate::pinmap::EncoderPins,
        tx: Sender<HardwareEvent>,
    ) -> Result<Self, String> {
        let gpio = Gpio::new().map_err(|e| e.to_string())?;

        let mut a = gpio
            .get(pins.a)
            .map_err(|e| e.to_string())?
            .into_input_pullup();
        let mut b = gpio
            .get(pins.b)
            .map_err(|e| e.to_string())?
            .into_input_pullup();
        let mut sw = gpio
            .get(pins.sw)
            .map_err(|e| e.to_string())?
            .into_input_pullup();

        let state = Arc::new(Mutex::new(QuadratureState::new(a.read(), b.read())));

        let state_a = state.clone();
        let tx_a = tx.clone();
        let id_a = id;
        a.set_async_interrupt(Trigger::Both, None, move |event_a| {
            if let Ok(mut state) = state_a.lock() {
                let b = state.last & 0b01;
                let next = (event_bit(event_a) << 1) | b;
                if let Some(delta) = state.update(next) {
                    let _ = tx_a.send(HardwareEvent::EncoderTurn { id: id_a, delta });
                }
            }
        })
        .map_err(|e| e.to_string())?;

        let state_b = state.clone();
        let tx_b = tx.clone();
        let id_b = id;
        b.set_async_interrupt(Trigger::Both, None, move |event_b| {
            if let Ok(mut state) = state_b.lock() {
                let a = state.last & 0b10;
                let next = a | event_bit(event_b);
                if let Some(delta) = state.update(next) {
                    let _ = tx_b.send(HardwareEvent::EncoderTurn { id: id_b, delta });
                }
            }
        })
        .map_err(|e| e.to_string())?;

        // Switch press (active low)
        let tx_sw = tx.clone();
        let id_sw = id;
        let last_press = Arc::new(Mutex::new(None::<Instant>));
        let debounce = Duration::from_millis(SWITCH_DEBOUNCE_MS);
        sw.set_async_interrupt(Trigger::FallingEdge, Some(debounce), move |_| {
            let now = Instant::now();
            if let Ok(mut last_press) = last_press.lock() {
                if last_press.is_some_and(|last| now.duration_since(last) < debounce) {
                    return;
                }
                *last_press = Some(now);
            }
            let _ = tx_sw.send(HardwareEvent::EncoderPress { id: id_sw });
        })
        .map_err(|e| e.to_string())?;

        Ok(Self {
            _id: id,
            _a: a,
            _b: b,
            _sw: sw,
            _state: state,
            _tx: tx,
        })
    }
}

#[cfg(feature = "pi-zero")]
fn levels_to_bits(a: Level, b: Level) -> u8 {
    (level_bit(a) << 1) | level_bit(b)
}

#[cfg(feature = "pi-zero")]
fn level_bit(level: Level) -> u8 {
    match level {
        Level::Low => 0,
        Level::High => 1,
    }
}

#[cfg(feature = "pi-zero")]
fn event_bit(event: Event) -> u8 {
    match event.trigger {
        Trigger::RisingEdge => 1,
        Trigger::FallingEdge => 0,
        _ => 0,
    }
}

/// Stub for non-Pi builds
#[cfg(not(feature = "pi-zero"))]
pub struct EncoderGpio {
    _private: (),
}

#[cfg(not(feature = "pi-zero"))]
impl EncoderGpio {
    pub fn new(
        _id: &'static str,
        _pins: &crate::pinmap::EncoderPins,
        _tx: Sender<HardwareEvent>,
    ) -> Result<Self, String> {
        Ok(Self { _private: () })
    }
}

#[cfg(not(feature = "pi-zero"))]
impl fmt::Debug for EncoderGpio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EncoderGpio {{ ... }}")
    }
}
