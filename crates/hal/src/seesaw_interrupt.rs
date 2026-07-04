#[cfg(feature = "pi-zero")]
use crate::pinmap::SEESAW_INT;
#[cfg(feature = "pi-zero")]
use rppal::gpio::{Gpio, InputPin, Level, Trigger};
#[cfg(feature = "pi-zero")]
use std::sync::mpsc::{self, Receiver};

#[cfg(feature = "pi-zero")]
pub struct SeesawInterrupt {
    pin: InputPin,
    rx: Receiver<()>,
}

#[cfg(feature = "pi-zero")]
impl SeesawInterrupt {
    pub fn new() -> Result<Self, String> {
        let gpio = Gpio::new().map_err(|e| format!("Seesaw INT GPIO init failed: {e}"))?;
        let mut pin = gpio
            .get(SEESAW_INT)
            .map_err(|e| format!("Seesaw INT GPIO{SEESAW_INT} open failed: {e}"))?
            .into_input_pullup();
        let (tx, rx) = mpsc::channel();
        pin.set_async_interrupt(Trigger::FallingEdge, None, move |_| {
            let _ = tx.send(());
        })
        .map_err(|e| format!("Seesaw INT interrupt init failed: {e}"))?;
        Ok(Self { pin, rx })
    }

    pub fn pending(&self) -> bool {
        let mut saw_event = false;
        while self.rx.try_recv().is_ok() {
            saw_event = true;
        }
        saw_event
    }

    pub fn asserted(&self) -> bool {
        self.pin.read() == Level::Low
    }
}

#[cfg(not(feature = "pi-zero"))]
#[derive(Default)]
pub struct SeesawInterrupt;

#[cfg(not(feature = "pi-zero"))]
impl SeesawInterrupt {
    pub fn new() -> Result<Self, String> {
        Ok(Self)
    }

    pub fn pending(&self) -> bool {
        false
    }

    pub fn asserted(&self) -> bool {
        false
    }
}
