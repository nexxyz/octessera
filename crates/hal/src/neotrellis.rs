//! NeoTrellis 8x8 LED matrix driver (4x4 devices x4 chain).

#[cfg(feature = "raspberry-pi-zero-2w")]
use crate::pinmap::TRELLIS_ADDRS;
#[cfg(test)]
use crate::seesaw_transport::NoopSeesawDelay;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w"))]
use crate::seesaw_transport::{LinuxSeesawTransport, ThreadSeesawDelay};
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
use crate::seesaw_transport::{SeesawDelay, SeesawTransport};
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
use std::time::Duration;

/// NeoTrellis device (4x4, daisy-chained to make 8x8).
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
pub struct NeoTrellis {
    devices: [(u16, [u8; 16]); 4],
    transports: [Box<dyn SeesawTransport>; 4],
    delay: Box<dyn SeesawDelay>,
    needs_setup: [bool; 4],
    led_refresh: bool,
}

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_STATUS_BASE: u8 = 0x00;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_HW_ID: u8 = 0x01;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_SW_RESET: u8 = 0x7F;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_KEYPAD_BASE: u8 = 0x10;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_KEYPAD_EVENT: u8 = 0x01;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_KEYPAD_INTENSET: u8 = 0x02;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_KEYPAD_COUNT: u8 = 0x04;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const SEESAW_KEYPAD_FIFO: u8 = 0x10;
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
const TRELLIS_NEOPIXEL_PIN: u8 = 3;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const TRELLIS_PIXELS_PER_DEVICE: usize = 16;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const TRELLIS_PIXEL_BYTES_PER_DEVICE: usize = TRELLIS_PIXELS_PER_DEVICE * 3;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const TRELLIS_LED_CHUNK_BYTES: usize = 24;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const TRELLIS_INIT_ATTEMPTS: usize = 3;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const TRELLIS_INIT_RETRY_DELAY: Duration = Duration::from_millis(250);
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const KEYPAD_EDGE_FALLING: u8 = 2;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const KEYPAD_EDGE_RISING: u8 = 3;

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
impl NeoTrellis {
    #[cfg(feature = "raspberry-pi-zero-2w")]
    pub fn new(i2c_path: &str) -> Result<Self, String> {
        Self::new_at(i2c_path, TRELLIS_ADDRS)
    }

    #[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w"))]
    pub fn new_at(i2c_path: &str, addresses: [u16; 4]) -> Result<Self, String> {
        let mut delay: Box<dyn SeesawDelay> = Box::new(ThreadSeesawDelay);
        let mut last_error = None;
        for attempt in 1..=TRELLIS_INIT_ATTEMPTS {
            let mut transports = match open_linux_transports(i2c_path, addresses) {
                Ok(transports) => transports,
                Err(error) => {
                    last_error = Some(error);
                    if attempt < TRELLIS_INIT_ATTEMPTS {
                        delay.sleep(TRELLIS_INIT_RETRY_DELAY);
                    }
                    continue;
                }
            };
            match initialize_once(addresses, &mut transports, &mut *delay) {
                Ok(()) => {
                    return Ok(Self {
                        devices: addresses.map(|address| (address, [0; 16])),
                        transports,
                        delay,
                        needs_setup: [false; 4],
                        led_refresh: false,
                    });
                }
                Err(error) => {
                    last_error = Some(error);
                    if attempt < TRELLIS_INIT_ATTEMPTS {
                        delay.sleep(TRELLIS_INIT_RETRY_DELAY);
                    }
                }
            }
        }
        Err(last_error.unwrap_or_else(|| "Trellis init failed".to_string()))
    }

    #[cfg(test)]
    pub(crate) fn from_transport(
        addresses: [u16; 4],
        transports: [Box<dyn SeesawTransport>; 4],
    ) -> Result<Self, String> {
        Self::initialize_with_retries(addresses, transports, Box::new(NoopSeesawDelay))
    }

    #[cfg(test)]
    fn initialize_with_retries(
        addresses: [u16; 4],
        mut transports: [Box<dyn SeesawTransport>; 4],
        mut delay: Box<dyn SeesawDelay>,
    ) -> Result<Self, String> {
        let mut last_error = None;
        for attempt in 1..=TRELLIS_INIT_ATTEMPTS {
            match initialize_once(addresses, &mut transports, &mut *delay) {
                Ok(()) => {
                    return Ok(Self {
                        devices: addresses.map(|address| (address, [0; 16])),
                        transports,
                        delay,
                        needs_setup: [false; 4],
                        led_refresh: false,
                    });
                }
                Err(error) => {
                    last_error = Some(error);
                    if attempt < TRELLIS_INIT_ATTEMPTS {
                        delay.sleep(TRELLIS_INIT_RETRY_DELAY);
                    }
                }
            }
        }
        Err(last_error.unwrap_or_else(|| "Trellis init failed".to_string()))
    }

    /// Append `(x, y, pressed)` in the lower-left coordinate space; on error, events already read
    /// from earlier boards stay in `events` because the Trellis reports each edge only once.
    pub fn scan_keys(&mut self, events: &mut Vec<(usize, usize, bool)>) -> Result<(), String> {
        for dev_idx in 0..self.devices.len() {
            if let Err(error) = self.scan_device(dev_idx, events) {
                self.needs_setup[dev_idx] = true;
                return Err(error);
            }
        }
        Ok(())
    }

    /// True once after a board's setup was re-applied, so the caller resends the LED frame.
    pub fn take_led_refresh(&mut self) -> bool {
        std::mem::take(&mut self.led_refresh)
    }

    fn scan_device(
        &mut self,
        dev_idx: usize,
        events: &mut Vec<(usize, usize, bool)>,
    ) -> Result<(), String> {
        if self.needs_setup[dev_idx] {
            self.restore_setup(dev_idx)?;
        }
        let mut count = [0_u8; 1];
        read_register(
            &mut *self.transports[dev_idx],
            SEESAW_KEYPAD_BASE,
            SEESAW_KEYPAD_COUNT,
            &mut count,
            "Trellis scan count failed",
        )?;
        let key_count = usize::from(count[0]).min(TRELLIS_PIXELS_PER_DEVICE);
        if key_count == 0 {
            return Ok(());
        }
        let mut buffer = [0_u8; 16];
        read_register(
            &mut *self.transports[dev_idx],
            SEESAW_KEYPAD_BASE,
            SEESAW_KEYPAD_FIFO,
            &mut buffer[..key_count],
            "Trellis scan FIFO failed",
        )?;
        for event in buffer.iter().take(key_count) {
            let Some((key, pressed)) = decode_trellis_key_event(*event) else {
                continue;
            };
            if let Some((x, y)) = trellis_coordinate(dev_idx, key) {
                events.push((x, y, pressed));
            }
        }
        Ok(())
    }

    /// No Seesaw register reads back its keypad or NeoPixel setup, so a brown-out reset is only
    /// visible as the board dropping off the bus. A board that failed a transaction gets its
    /// setup re-applied, without another reset, once it answers again.
    fn restore_setup(&mut self, dev_idx: usize) -> Result<(), String> {
        let address = self.devices[dev_idx].0;
        configure_device(address, &mut *self.transports[dev_idx])?;
        self.needs_setup[dev_idx] = false;
        self.led_refresh = true;
        eprintln!(
            "NeoTrellis {address:#04x} answered again after an I2C failure; setup re-applied"
        );
        Ok(())
    }

    /// A board that soft-resets quickly answers every transaction and has no reset flag, so the
    /// caller re-applies each board's keypad and NeoPixel setup periodically, one short part per
    /// call so a key scan never waits long behind it. The firmware treats every setup write as
    /// idempotent and none touches the pixel buffer or the LEDs. Returns whether parts remain.
    pub fn reapply_setup_part(&mut self, dev_idx: usize, part: usize) -> Result<bool, String> {
        let first = part * SETUP_WRITES_PER_PART;
        let end = (first + SETUP_WRITES_PER_PART).min(TRELLIS_SETUP_WRITES);
        for index in first..end {
            write_setup(&mut *self.transports[dev_idx], index)
                .inspect_err(|_| self.needs_setup[dev_idx] = true)?;
        }
        Ok(end < TRELLIS_SETUP_WRITES)
    }

    /// Write an 8x8 RGB frame to the four NeoPixel buffers, then latch all four.
    pub fn write_led_frame(&mut self, frame: &[[u8; 3]; 64]) -> Result<(), String> {
        for dev_idx in 0..self.devices.len() {
            let base_x = (dev_idx % 2) * 4;
            let base_y = (dev_idx / 2) * 4;
            let mut data = Vec::with_capacity(TRELLIS_PIXEL_BYTES_PER_DEVICE);
            for y in base_y..(base_y + 4) {
                for x in base_x..(base_x + 4) {
                    data.extend_from_slice(&grb_color(frame[y * 8 + x]));
                }
            }
            write_led_buffer_chunks(&mut *self.transports[dev_idx], &data)
                .inspect_err(|_| self.needs_setup[dev_idx] = true)?;
        }
        for (dev_idx, transport) in self.transports.iter_mut().enumerate() {
            write_register(
                &mut **transport,
                SEESAW_NEOPIXEL_BASE,
                SEESAW_NEOPIXEL_SHOW,
                &[],
                "Trellis LED show failed",
            )
            .inspect_err(|_| self.needs_setup[dev_idx] = true)?;
        }
        self.delay.sleep(TRELLIS_SHOW_SETTLE);
        Ok(())
    }
}

/// A Seesaw refreshing its 16 NeoPixels (about 30 µs each plus the latch gap) answers I2C late and
/// stretches the clock; the Raspberry's I2C controller mishandles stretching and can corrupt the
/// transfer and hang the chip until power-off. Only the addressed chip stretches, so the other
/// boards' SHOWs may follow at once; the bus then stays idle until every refresh is done.
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
const TRELLIS_SHOW_SETTLE: std::time::Duration = std::time::Duration::from_millis(1);

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w"))]
fn open_linux_transports(
    i2c_path: &str,
    addresses: [u16; 4],
) -> Result<[Box<dyn SeesawTransport>; 4], String> {
    Ok([
        Box::new(LinuxSeesawTransport::new(
            i2c_path,
            addresses[0],
            "NeoTrellis",
        )?) as Box<dyn SeesawTransport>,
        Box::new(LinuxSeesawTransport::new(
            i2c_path,
            addresses[1],
            "NeoTrellis",
        )?) as Box<dyn SeesawTransport>,
        Box::new(LinuxSeesawTransport::new(
            i2c_path,
            addresses[2],
            "NeoTrellis",
        )?) as Box<dyn SeesawTransport>,
        Box::new(LinuxSeesawTransport::new(
            i2c_path,
            addresses[3],
            "NeoTrellis",
        )?) as Box<dyn SeesawTransport>,
    ])
}

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
fn initialize_once(
    addresses: [u16; 4],
    transports: &mut [Box<dyn SeesawTransport>; 4],
    delay: &mut dyn SeesawDelay,
) -> Result<(), String> {
    for transport in transports.iter_mut() {
        write_register(
            &mut **transport,
            SEESAW_STATUS_BASE,
            SEESAW_SW_RESET,
            &[0xFF],
            "Trellis reset failed",
        )?;
    }
    delay.sleep(Duration::from_millis(500));

    for (address, transport) in addresses.iter().copied().zip(transports.iter_mut()) {
        configure_device(address, &mut **transport)?;
    }
    Ok(())
}

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
fn trellis_coordinate(device_index: usize, key: u8) -> Option<(usize, usize)> {
    (key < TRELLIS_PIXELS_PER_DEVICE as u8).then(|| {
        let base_x = (device_index % 2) * 4;
        let base_y = (device_index / 2) * 4;
        (
            base_x + usize::from(key % 4),
            7 - (base_y + usize::from(key / 4)),
        )
    })
}

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
fn grb_color(rgb: [u8; 3]) -> [u8; 3] {
    [rgb[1], rgb[0], rgb[2]]
}

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
fn decode_trellis_key_event(key_data: u8) -> Option<(u8, bool)> {
    let edge = key_data & 0x03;
    if !matches!(edge, KEYPAD_EDGE_FALLING | KEYPAD_EDGE_RISING) {
        return None;
    }
    Some((
        seesaw_key_to_trellis_key(key_data >> 2),
        edge == KEYPAD_EDGE_RISING,
    ))
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

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
fn write_led_buffer_chunks(transport: &mut dyn SeesawTransport, data: &[u8]) -> Result<(), String> {
    for offset in (0..data.len()).step_by(TRELLIS_LED_CHUNK_BYTES) {
        let end = (offset + TRELLIS_LED_CHUNK_BYTES).min(data.len());
        let mut chunk = Vec::with_capacity(2 + end - offset);
        chunk.extend_from_slice(&(offset as u16).to_be_bytes());
        chunk.extend_from_slice(&data[offset..end]);
        write_register(
            transport,
            SEESAW_NEOPIXEL_BASE,
            SEESAW_NEOPIXEL_BUF,
            &chunk,
            "Trellis LED buffer write failed",
        )?;
    }
    Ok(())
}

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
fn trellis_key_to_seesaw_key(key: u8) -> u8 {
    ((key / 4) * 8) + (key % 4)
}

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
fn seesaw_key_to_trellis_key(key: u8) -> u8 {
    ((key / 8) * 4) + (key % 8)
}

#[cfg(not(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test)))]
pub struct NeoTrellis {
    _private: (),
}

#[cfg(not(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test)))]
impl NeoTrellis {
    pub fn new(_i2c_path: &str) -> Result<Self, String> {
        Ok(Self { _private: () })
    }

    pub fn scan_keys(&mut self, _events: &mut Vec<(usize, usize, bool)>) -> Result<(), String> {
        Ok(())
    }

    pub fn take_led_refresh(&mut self) -> bool {
        false
    }

    pub fn reapply_setup_part(&mut self, _dev_idx: usize, _part: usize) -> Result<bool, String> {
        Ok(false)
    }

    pub fn write_led_frame(&mut self, _frame: &[[u8; 3]; 64]) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(not(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test)))]
impl std::fmt::Debug for NeoTrellis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NeoTrellis {{ ... }}")
    }
}

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
mod setup;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
use setup::{configure_device, write_setup, SETUP_WRITES_PER_PART, TRELLIS_SETUP_WRITES};

#[cfg(test)]
mod tests;
