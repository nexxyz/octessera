#[cfg(feature = "raspberry-pi-zero-2w")]
use crate::pinmap::NEOKEY_ADDR;
#[cfg(test)]
use crate::seesaw_transport::NoopSeesawDelay;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w"))]
use crate::seesaw_transport::{LinuxSeesawTransport, ThreadSeesawDelay};
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
use crate::seesaw_transport::{SeesawDelay, SeesawTransport};
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
use std::time::{Duration, Instant};

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
pub struct NeoKey {
    transport: Box<dyn SeesawTransport>,
    debouncer: NeoKeyDebouncer,
    delay: Box<dyn SeesawDelay>,
    needs_setup: bool,
    led_refresh: bool,
}

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_STATUS_BASE: u8 = 0x00;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_HW_ID: u8 = 0x01;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_SW_RESET: u8 = 0x7F;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_GPIO_BASE: u8 = 0x01;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_GPIO_DIRCLR_BULK: u8 = 0x03;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_GPIO_BULK: u8 = 0x04;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_GPIO_BULK_SET: u8 = 0x05;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_GPIO_INTENSET: u8 = 0x08;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_GPIO_INTFLAG: u8 = 0x0A;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_GPIO_PULLENSET: u8 = 0x0B;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_NEOPIXEL_BASE: u8 = 0x0E;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_NEOPIXEL_PIN: u8 = 0x01;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_NEOPIXEL_BUF_LENGTH: u8 = 0x03;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_NEOPIXEL_BUF: u8 = 0x04;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_NEOPIXEL_SHOW: u8 = 0x05;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const NEOKEY_BUTTON_MASK: u32 = 0xF0;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const NEOKEY_NEOPIXEL_PIN: u8 = 3;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const NEOKEY_LED_BYTES: u16 = 12;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const NEOKEY_INIT_ATTEMPTS: usize = 3;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const NEOKEY_INIT_RETRY_DELAY: Duration = Duration::from_millis(250);

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
mod debounce;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
use debounce::NeoKeyDebouncer;
#[cfg(test)]
use debounce::NEOKEY_DEBOUNCE;

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
impl NeoKey {
    #[cfg(feature = "raspberry-pi-zero-2w")]
    pub fn new(i2c_path: &str) -> Result<Self, String> {
        Self::new_at(i2c_path, NEOKEY_ADDR)
    }

    #[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w"))]
    pub fn new_at(i2c_path: &str, address: u16) -> Result<Self, String> {
        let mut delay: Box<dyn SeesawDelay> = Box::new(ThreadSeesawDelay);
        let mut last_error = None;
        for attempt in 1..=NEOKEY_INIT_ATTEMPTS {
            let mut transport: Box<dyn SeesawTransport> =
                match LinuxSeesawTransport::new(i2c_path, address, "NeoKey") {
                    Ok(transport) => Box::new(transport),
                    Err(error) => {
                        last_error = Some(error);
                        if attempt < NEOKEY_INIT_ATTEMPTS {
                            delay.sleep(NEOKEY_INIT_RETRY_DELAY);
                        }
                        continue;
                    }
                };
            match initialize_once(&mut *transport, &mut *delay) {
                Ok(()) => {
                    return Ok(Self {
                        transport,
                        debouncer: NeoKeyDebouncer::default(),
                        delay,
                        needs_setup: false,
                        led_refresh: false,
                    });
                }
                Err(error) => {
                    last_error = Some(error);
                    if attempt < NEOKEY_INIT_ATTEMPTS {
                        delay.sleep(NEOKEY_INIT_RETRY_DELAY);
                    }
                }
            }
        }
        Err(last_error.unwrap_or_else(|| "NeoKey init failed".to_string()))
    }

    #[cfg(test)]
    pub(crate) fn from_transport(transport: Box<dyn SeesawTransport>) -> Result<Self, String> {
        Self::initialize_with_retries(transport, Box::new(NoopSeesawDelay))
    }

    #[cfg(test)]
    fn initialize_with_retries(
        mut transport: Box<dyn SeesawTransport>,
        mut delay: Box<dyn SeesawDelay>,
    ) -> Result<Self, String> {
        let mut last_error = None;
        for attempt in 1..=NEOKEY_INIT_ATTEMPTS {
            match initialize_once(&mut *transport, &mut *delay) {
                Ok(()) => {
                    return Ok(Self {
                        transport,
                        debouncer: NeoKeyDebouncer::default(),
                        delay,
                        needs_setup: false,
                        led_refresh: false,
                    });
                }
                Err(error) => {
                    last_error = Some(error);
                    if attempt < NEOKEY_INIT_ATTEMPTS {
                        delay.sleep(NEOKEY_INIT_RETRY_DELAY);
                    }
                }
            }
        }
        Err(last_error.unwrap_or_else(|| "NeoKey init failed".to_string()))
    }

    pub fn scan(&mut self) -> Result<Vec<(u8, bool)>, String> {
        let sampled = neokey_buttons_from_raw(self.raw_button_state()?);
        let stable_buttons = self.debouncer.update(sampled, Instant::now());
        Ok((0..4)
            .map(|key| (key, stable_buttons[usize::from(key)]))
            .collect())
    }

    pub fn settling(&self) -> bool {
        self.debouncer.settling()
    }

    /// No Seesaw register reads back the GPIO or NeoPixel setup, so a brown-out reset is only
    /// visible as the board dropping off the bus. After a failed transaction the setup is
    /// re-applied, without another reset, once the board answers again.
    pub fn scan_interrupts(&mut self) -> Result<Vec<(u8, bool)>, String> {
        let result = self.restore_setup_if_needed().and_then(|()| {
            self.clear_interrupt_flags()?;
            self.scan()
        });
        self.needs_setup |= result.is_err();
        result
    }

    /// True once after the setup was re-applied, so the caller resends the key colors.
    pub fn take_led_refresh(&mut self) -> bool {
        std::mem::take(&mut self.led_refresh)
    }

    fn restore_setup_if_needed(&mut self) -> Result<(), String> {
        if !self.needs_setup {
            return Ok(());
        }
        configure(&mut *self.transport)?;
        self.needs_setup = false;
        self.led_refresh = true;
        eprintln!("NeoKey answered again after an I2C failure; setup re-applied");
        Ok(())
    }

    pub fn raw_button_state(&mut self) -> Result<u32, String> {
        let mut buffer = [0_u8; 4];
        read_register(
            &mut *self.transport,
            SEESAW_GPIO_BASE,
            SEESAW_GPIO_BULK,
            &mut buffer,
            "NeoKey raw scan failed",
        )?;
        Ok(u32::from_be_bytes(buffer))
    }

    fn clear_interrupt_flags(&mut self) -> Result<(), String> {
        let mut buffer = [0_u8; 4];
        read_register(
            &mut *self.transport,
            SEESAW_GPIO_BASE,
            SEESAW_GPIO_INTFLAG,
            &mut buffer,
            "NeoKey GPIO interrupt clear failed",
        )
    }

    pub fn set_led(&mut self, key: u8, r: u8, g: u8, b: u8) -> Result<(), String> {
        if key >= 4 {
            return Err(format!("NeoKey LED index out of range: {key}"));
        }
        let result = self.write_led(key, [g, r, b]);
        self.needs_setup |= result.is_err();
        result
    }

    fn write_led(&mut self, key: u8, grb: [u8; 3]) -> Result<(), String> {
        let offset = u16::from(key) * 3;
        let mut data = Vec::with_capacity(5);
        data.extend_from_slice(&offset.to_be_bytes());
        data.extend_from_slice(&grb);
        write_register(
            &mut *self.transport,
            SEESAW_NEOPIXEL_BASE,
            SEESAW_NEOPIXEL_BUF,
            &data,
            "NeoKey LED write failed",
        )?;
        write_register(
            &mut *self.transport,
            SEESAW_NEOPIXEL_BASE,
            SEESAW_NEOPIXEL_SHOW,
            &[],
            "NeoKey LED show failed",
        )?;
        self.delay.sleep(Duration::from_micros(300));
        Ok(())
    }
}

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
fn initialize_once(
    transport: &mut dyn SeesawTransport,
    delay: &mut dyn SeesawDelay,
) -> Result<(), String> {
    write_register(
        transport,
        SEESAW_STATUS_BASE,
        SEESAW_SW_RESET,
        &[0xFF],
        "NeoKey reset failed",
    )?;
    delay.sleep(Duration::from_millis(500));
    configure(transport)
}

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
fn configure(transport: &mut dyn SeesawTransport) -> Result<(), String> {
    let mask = NEOKEY_BUTTON_MASK.to_be_bytes();
    let led_bytes = NEOKEY_LED_BYTES.to_be_bytes();
    let mut hw_id = [0_u8; 1];
    read_register(
        transport,
        SEESAW_STATUS_BASE,
        SEESAW_HW_ID,
        &mut hw_id,
        "NeoKey HW ID read failed",
    )?;
    if !matches!(hw_id[0], 0x55 | 0x84..=0x89) {
        return Err(format!("NeoKey HW ID invalid: {:#04x}", hw_id[0]));
    }

    write_register(
        transport,
        SEESAW_GPIO_BASE,
        SEESAW_GPIO_DIRCLR_BULK,
        &mask,
        "NeoKey GPIO direction init failed",
    )?;
    write_register(
        transport,
        SEESAW_GPIO_BASE,
        SEESAW_GPIO_PULLENSET,
        &mask,
        "NeoKey GPIO pullup init failed",
    )?;
    write_register(
        transport,
        SEESAW_GPIO_BASE,
        SEESAW_GPIO_BULK_SET,
        &mask,
        "NeoKey GPIO pullup set failed",
    )?;
    write_register(
        transport,
        SEESAW_GPIO_BASE,
        SEESAW_GPIO_INTENSET,
        &mask,
        "NeoKey GPIO interrupt init failed",
    )?;
    let mut flags = [0_u8; 4];
    read_register(
        transport,
        SEESAW_GPIO_BASE,
        SEESAW_GPIO_INTFLAG,
        &mut flags,
        "NeoKey GPIO interrupt clear failed",
    )?;
    write_register(
        transport,
        SEESAW_NEOPIXEL_BASE,
        SEESAW_NEOPIXEL_PIN,
        &[NEOKEY_NEOPIXEL_PIN],
        "NeoKey LED pin init failed",
    )?;
    write_register(
        transport,
        SEESAW_NEOPIXEL_BASE,
        SEESAW_NEOPIXEL_BUF_LENGTH,
        &led_bytes,
        "NeoKey LED length init failed",
    )
}

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
fn neokey_buttons_from_raw(raw: u32) -> [bool; 4] {
    let state = raw & NEOKEY_BUTTON_MASK;
    [
        (state & (1 << 4)) == 0,
        (state & (1 << 5)) == 0,
        (state & (1 << 6)) == 0,
        (state & (1 << 7)) == 0,
    ]
}

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
fn write_register(
    transport: &mut dyn SeesawTransport,
    base: u8,
    function: u8,
    data: &[u8],
    context: &str,
) -> Result<(), String> {
    transport
        .write(base, function, data)
        .map_err(|error| format!("{context}: {error}"))
}

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
fn read_register(
    transport: &mut dyn SeesawTransport,
    base: u8,
    function: u8,
    buffer: &mut [u8],
    context: &str,
) -> Result<(), String> {
    transport
        .read(base, function, buffer)
        .map_err(|error| format!("{context}: {error}"))
}

#[cfg(not(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test)))]
pub struct NeoKey {
    _private: (),
}

#[cfg(not(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test)))]
impl NeoKey {
    pub fn new(_i2c_path: &str) -> Result<Self, String> {
        Ok(Self { _private: () })
    }

    pub fn scan(&mut self) -> Result<Vec<(u8, bool)>, String> {
        Ok(Vec::new())
    }

    pub fn settling(&self) -> bool {
        false
    }

    pub fn scan_interrupts(&mut self) -> Result<Vec<(u8, bool)>, String> {
        self.scan()
    }

    pub fn take_led_refresh(&mut self) -> bool {
        false
    }

    pub fn raw_button_state(&mut self) -> Result<u32, String> {
        Ok(0)
    }

    pub fn set_led(&mut self, _key: u8, _r: u8, _g: u8, _b: u8) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(not(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test)))]
impl std::fmt::Debug for NeoKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NeoKey {{ ... }}")
    }
}

#[cfg(test)]
mod tests;
