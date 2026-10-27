#[cfg(feature = "raspberry-pi-zero-2w")]
use crate::pinmap::SEESAW_INT;
#[cfg(feature = "raspberry-pi-zero-2w")]
use rppal::gpio::{Gpio, InputPin, Level, Trigger};
#[cfg(feature = "raspberry-pi-zero-2w")]
use std::sync::mpsc::{self, Receiver};

#[cfg(feature = "raspberry-pi-zero-2w")]
pub struct SeesawInterrupt {
    pin: InputPin,
    rx: Receiver<()>,
}

#[cfg(feature = "raspberry-pi-zero-2w")]
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

#[cfg(all(feature = "orange-pi-zero-2w", target_os = "linux"))]
pub struct SeesawInterrupt {
    request: gpiocdev::Request,
    offset: u32,
}

#[cfg(all(feature = "orange-pi-zero-2w", target_os = "linux"))]
impl SeesawInterrupt {
    pub fn new() -> Result<Self, String> {
        use crate::board_profiles::ORANGE_PI_ZERO_2W_DEVICES;
        use gpiocdev::line::{Bias, EdgeDetection};
        let devices = ORANGE_PI_ZERO_2W_DEVICES;
        let offset = devices.seesaw_int;
        let chip_path =
            crate::orange_encoder_gpio::find_gpio_chip(devices.gpio.chip_label, &[offset])?;
        let request = gpiocdev::Request::builder()
            .on_chip(&chip_path)
            .with_consumer("octessera-seesaw-int")
            .with_line(offset)
            .as_input()
            .with_bias(Bias::PullUp)
            .with_edge_detection(EdgeDetection::FallingEdge)
            .request()
            .map_err(|e| {
                format!("Seesaw INT PH1 line {offset} request failed (UART0 active?): {e}")
            })?;
        Ok(Self { request, offset })
    }

    pub fn pending(&self) -> bool {
        let mut saw_event = false;
        while matches!(self.request.has_edge_event(), Ok(true)) {
            saw_event = true;
            if self.request.read_edge_event().is_err() {
                break;
            }
        }
        saw_event
    }

    pub fn asserted(&self) -> bool {
        matches!(
            self.request.value(self.offset),
            Ok(gpiocdev::line::Value::Inactive)
        )
    }
}

#[cfg(not(any(
    feature = "raspberry-pi-zero-2w",
    all(feature = "orange-pi-zero-2w", target_os = "linux")
)))]
#[derive(Default)]
pub struct SeesawInterrupt;

#[cfg(not(any(
    feature = "raspberry-pi-zero-2w",
    all(feature = "orange-pi-zero-2w", target_os = "linux")
)))]
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
