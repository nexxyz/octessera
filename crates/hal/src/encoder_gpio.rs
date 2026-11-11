//! Rotary encoder input via GPIO (quadrature + push switch)
//! Uses rppal for interrupt-driven decoding on Pi Zero 2W

#[cfg(feature = "raspberry-pi-zero-2w")]
use rppal::gpio::{Gpio, InputPin, Level, Trigger};
#[cfg(not(feature = "orange-pi-zero-2w"))]
use std::sync::mpsc::Sender;
#[cfg(feature = "raspberry-pi-zero-2w")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "raspberry-pi-zero-2w")]
use std::time::Duration;

#[cfg(all(
    not(feature = "raspberry-pi-zero-2w"),
    not(feature = "orange-pi-zero-2w")
))]
use std::fmt;

/// Hardware event from encoders
#[derive(Debug, Clone, Copy)]
pub enum HardwareEvent {
    EncoderTurn { id: &'static str, delta: i8 },
    EncoderPress { id: &'static str },
    EncoderRelease { id: &'static str },
}

#[cfg(any(
    feature = "raspberry-pi-zero-2w",
    all(feature = "orange-pi-zero-2w", target_os = "linux")
))]
pub(crate) const SWITCH_DEBOUNCE_MS: u64 = 45;

/// Rotary encoder with GPIO interrupt handling
#[cfg(feature = "raspberry-pi-zero-2w")]
pub struct EncoderGpio {
    _sw: InputPin,
    quadrature: Arc<Mutex<Option<QuadratureLines>>>,
}

/// Both lines live behind the decoder lock so every edge on either line reads both levels.
#[cfg(feature = "raspberry-pi-zero-2w")]
struct QuadratureLines {
    a: InputPin,
    b: InputPin,
    state: QuadratureState,
}

#[cfg(any(
    feature = "raspberry-pi-zero-2w",
    all(feature = "orange-pi-zero-2w", target_os = "linux"),
    test
))]
pub(crate) struct QuadratureState {
    rest: u8,
    last: u8,
    accum: i8,
}

/// Each detent is one full quadrature cycle (four transitions) that starts and ends at the level
/// pair the encoder rests in, read at startup while the detent holds the shaft. Valid transitions accumulate signed, so contact bounce cancels out;
/// a two-bit jump means a missed edge and counts two in the direction already under way. A detent
/// is emitted only when the levels return to rest, rounded to whole detents.
#[cfg(any(
    feature = "raspberry-pi-zero-2w",
    all(feature = "orange-pi-zero-2w", target_os = "linux"),
    test
))]
impl QuadratureState {
    pub(crate) fn new_bits(bits: u8) -> Self {
        Self {
            rest: bits & 0b11,
            last: bits & 0b11,
            accum: 0,
        }
    }

    pub(crate) fn update(&mut self, next: u8) -> Option<i8> {
        let next = next & 0b11;
        let step = match (self.last << 2) | next {
            0b0001 | 0b0111 | 0b1110 | 0b1000 => 1,
            0b0010 | 0b1011 | 0b1101 | 0b0100 => -1,
            0b0011 | 0b1100 | 0b0110 | 0b1001 => self.accum.signum() * 2,
            _ => 0,
        };
        self.last = next;
        self.accum = self.accum.saturating_add(step);
        if next != self.rest {
            return None;
        }
        let accum = std::mem::take(&mut self.accum);
        (accum.abs() >= 2).then_some(accum.saturating_add(accum.signum() * 2) / 4)
    }
}

#[cfg(feature = "raspberry-pi-zero-2w")]
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

        let quadrature = Arc::new(Mutex::new(None::<QuadratureLines>));
        for pin in [&mut a, &mut b] {
            let lines = Arc::clone(&quadrature);
            let tx = tx.clone();
            pin.set_async_interrupt(Trigger::Both, None, move |_| {
                let Ok(mut lines) = lines.lock() else {
                    return;
                };
                let Some(lines) = lines.as_mut() else {
                    return;
                };
                let next = levels_to_bits(lines.a.read(), lines.b.read());
                if let Some(delta) = lines.state.update(next) {
                    let _ = tx.send(HardwareEvent::EncoderTurn { id, delta });
                }
            })
            .map_err(|e| e.to_string())?;
        }
        let state = QuadratureState::new_bits(levels_to_bits(a.read(), b.read()));
        *quadrature
            .lock()
            .map_err(|_| "encoder decoder mutex poisoned".to_string())? =
            Some(QuadratureLines { a, b, state });

        // Switch press/release (active low)
        let tx_sw = tx;
        let id_sw = id;
        let debounce = Duration::from_millis(SWITCH_DEBOUNCE_MS);
        sw.set_async_interrupt(Trigger::Both, Some(debounce), move |event| {
            let message = match event.trigger {
                Trigger::FallingEdge => HardwareEvent::EncoderPress { id: id_sw },
                Trigger::RisingEdge => HardwareEvent::EncoderRelease { id: id_sw },
                _ => return,
            };
            let _ = tx_sw.send(message);
        })
        .map_err(|e| e.to_string())?;

        Ok(Self {
            _sw: sw,
            quadrature,
        })
    }
}

#[cfg(feature = "raspberry-pi-zero-2w")]
impl Drop for EncoderGpio {
    fn drop(&mut self) {
        // Drop the pins outside the lock: dropping them joins the interrupt threads that take it.
        let lines = self
            .quadrature
            .lock()
            .ok()
            .and_then(|mut lines| lines.take());
        drop(lines);
    }
}

#[cfg(feature = "raspberry-pi-zero-2w")]
fn levels_to_bits(a: Level, b: Level) -> u8 {
    (level_bit(a) << 1) | level_bit(b)
}

#[cfg(feature = "raspberry-pi-zero-2w")]
fn level_bit(level: Level) -> u8 {
    match level {
        Level::Low => 0,
        Level::High => 1,
    }
}

/// Stub for non-Pi builds
#[cfg(all(
    not(feature = "raspberry-pi-zero-2w"),
    not(feature = "orange-pi-zero-2w")
))]
pub struct EncoderGpio {
    _private: (),
}

#[cfg(all(
    not(feature = "raspberry-pi-zero-2w"),
    not(feature = "orange-pi-zero-2w")
))]
impl EncoderGpio {
    pub fn new(
        _id: &'static str,
        _pins: &crate::pinmap::EncoderPins,
        _tx: Sender<HardwareEvent>,
    ) -> Result<Self, String> {
        Ok(Self { _private: () })
    }
}

#[cfg(all(
    not(feature = "raspberry-pi-zero-2w"),
    not(feature = "orange-pi-zero-2w")
))]
impl fmt::Debug for EncoderGpio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EncoderGpio {{ ... }}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CW: [u8; 4] = [0b01, 0b11, 0b10, 0b00];
    const CCW: [u8; 4] = [0b10, 0b11, 0b01, 0b00];

    fn feed(state: &mut QuadratureState, levels: &[u8]) -> Vec<i8> {
        levels
            .iter()
            .filter_map(|bits| state.update(*bits))
            .collect()
    }

    fn with_bounce(detent: [u8; 4]) -> Vec<u8> {
        let mut levels = Vec::new();
        let mut previous = 0b00;
        for next in detent {
            levels.extend([next, previous, next, previous, next]);
            previous = next;
        }
        levels
    }

    #[test]
    fn quadrature_emits_after_four_clean_edges() {
        let mut state = QuadratureState::new_bits(0b00);

        assert_eq!(state.update(0b01), None);
        assert_eq!(state.update(0b11), None);
        assert_eq!(state.update(0b10), None);
        assert_eq!(state.update(0b00), Some(1));
    }

    #[test]
    fn a_detent_counts_only_when_the_encoder_returns_to_rest() {
        let mut state = QuadratureState::new_bits(0b00);

        assert_eq!(
            feed(&mut state, &[0b01, 0b00, 0b10, 0b11, 0b01]),
            Vec::<i8>::new()
        );
        assert_eq!(state.update(0b00), Some(-1));
    }

    #[test]
    fn a_slow_bouncing_detent_emits_exactly_one_step_each_way() {
        let mut state = QuadratureState::new_bits(0b00);

        assert_eq!(feed(&mut state, &with_bounce(CW)), vec![1]);
        assert_eq!(feed(&mut state, &with_bounce(CCW)), vec![-1]);
        assert_eq!(feed(&mut state, &with_bounce(CW)), vec![1]);
    }

    #[test]
    fn contact_chatter_at_rest_emits_nothing() {
        let mut state = QuadratureState::new_bits(0b00);

        assert_eq!(
            feed(&mut state, &[0b01, 0b00, 0b10, 0b00, 0b01, 0b00]),
            Vec::<i8>::new()
        );
        assert_eq!(feed(&mut state, &CW), vec![1]);
    }

    #[test]
    fn stop_and_go_turns_count_whole_detents_only() {
        let mut state = QuadratureState::new_bits(0b00);

        assert_eq!(feed(&mut state, &[0b01, 0b11]), Vec::<i8>::new());
        assert_eq!(feed(&mut state, &[0b10, 0b00]), vec![1]);
        assert_eq!(
            feed(&mut state, &[0b01, 0b11, 0b01, 0b00]),
            Vec::<i8>::new()
        );
        assert_eq!(feed(&mut state, &[0b10, 0b11]), Vec::<i8>::new());
        assert_eq!(feed(&mut state, &[0b01, 0b00]), vec![-1]);
    }

    #[test]
    fn a_fast_turn_with_a_missed_edge_in_every_detent_keeps_every_detent() {
        let mut state = QuadratureState::new_bits(0b00);
        let missed_middle = [0b01, 0b10, 0b00];
        let missed_first = [0b11, 0b10, 0b00];
        let missed_last = [0b01, 0b11, 0b00];

        assert_eq!(
            feed(
                &mut state,
                &[missed_middle, missed_first, missed_last, missed_middle].concat()
            ),
            vec![1, 1, 1, 1]
        );
        assert_eq!(feed(&mut state, &[0b10, 0b01, 0b00]), vec![-1]);
    }

    #[test]
    fn a_missed_rest_crossing_keeps_both_detents() {
        let mut state = QuadratureState::new_bits(0b00);

        assert_eq!(
            feed(&mut state, &[0b01, 0b11, 0b10, 0b01, 0b11, 0b10, 0b00]),
            vec![2]
        );
    }

    #[test]
    fn rest_is_the_level_pair_the_encoder_started_in() {
        let mut state = QuadratureState::new_bits(0b11);

        assert_eq!(feed(&mut state, &[0b10, 0b00, 0b01]), Vec::<i8>::new());
        assert_eq!(state.update(0b11), Some(1));
    }
}
