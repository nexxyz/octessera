#[cfg(feature = "raspberry-pi-zero-2w")]
use crate::pinmap::TRELLIS_ADDRS;
#[cfg(test)]
use crate::seesaw_transport::NoopSeesawDelay;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w"))]
use crate::seesaw_transport::{LinuxSeesawTransport, ThreadSeesawDelay};
use crate::seesaw_transport::{SeesawDelay, SeesawTransport};
use std::time::Duration;

/// NeoTrellis device (4x4, daisy-chained to make 8x8).
pub struct NeoTrellis {
    devices: [(u16, [u8; 16]); 4],
    transports: [Box<dyn SeesawTransport>; 4],
    delay: Box<dyn SeesawDelay>,
    needs_setup: [bool; 4],
    led_refresh: bool,
    /// Each board's GRB pixel buffer as last written, for the setup refresh to send again.
    pixels: [[u8; TRELLIS_PIXEL_BYTES_PER_DEVICE]; 4],
    /// Whether the board's LEDs show `pixels`; until then a frame rewrites it even if unchanged.
    shown: [bool; 4],
}

const SEESAW_STATUS_BASE: u8 = 0x00;
const SEESAW_HW_ID: u8 = 0x01;
const SEESAW_SW_RESET: u8 = 0x7F;
const SEESAW_KEYPAD_BASE: u8 = 0x10;
const SEESAW_KEYPAD_EVENT: u8 = 0x01;
const SEESAW_KEYPAD_INTENSET: u8 = 0x02;
const SEESAW_KEYPAD_COUNT: u8 = 0x04;
const SEESAW_KEYPAD_FIFO: u8 = 0x10;
const SEESAW_NEOPIXEL_BASE: u8 = 0x0E;
const SEESAW_NEOPIXEL_PIN: u8 = 0x01;
const SEESAW_NEOPIXEL_BUF_LENGTH: u8 = 0x03;
const SEESAW_NEOPIXEL_BUF: u8 = 0x04;
const SEESAW_NEOPIXEL_SHOW: u8 = 0x05;
const TRELLIS_NEOPIXEL_PIN: u8 = 3;
const TRELLIS_PIXELS_PER_DEVICE: usize = 16;
const TRELLIS_PIXEL_BYTES_PER_DEVICE: usize = TRELLIS_PIXELS_PER_DEVICE * 3;
const TRELLIS_LED_CHUNK_BYTES: usize = 24;
const TRELLIS_LED_CHUNKS: usize = TRELLIS_PIXEL_BYTES_PER_DEVICE.div_ceil(TRELLIS_LED_CHUNK_BYTES);
const TRELLIS_INIT_ATTEMPTS: usize = 3;
const TRELLIS_INIT_RETRY_DELAY: Duration = Duration::from_millis(250);
const KEYPAD_EDGE_FALLING: u8 = 2;
const KEYPAD_EDGE_RISING: u8 = 3;

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
                        pixels: [[0; TRELLIS_PIXEL_BYTES_PER_DEVICE]; 4],
                        shown: [false; 4],
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
                        pixels: [[0; TRELLIS_PIXEL_BYTES_PER_DEVICE]; 4],
                        shown: [false; 4],
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
        self.shown[dev_idx] = false;
        self.led_refresh = true;
        eprintln!(
            "NeoTrellis {address:#04x} answered again after an I2C failure; setup re-applied"
        );
        Ok(())
    }

    /// A board that soft-resets quickly answers every transaction and has no reset flag, so the
    /// caller re-applies each board's keypad and NeoPixel setup periodically, one short part per
    /// call so a key scan never waits long behind it. The firmware treats every setup write as
    /// idempotent. A reset also cleared the pixel buffer, so the last parts write the board's
    /// pixels again, one chunk per part, and then latch them. Returns whether parts remain.
    pub fn reapply_setup_part(&mut self, dev_idx: usize, part: usize) -> Result<bool, String> {
        let setup_parts = TRELLIS_SETUP_WRITES.div_ceil(SETUP_WRITES_PER_PART);
        let transport = &mut *self.transports[dev_idx];
        let result = if part < setup_parts {
            let first = part * SETUP_WRITES_PER_PART;
            let end = (first + SETUP_WRITES_PER_PART).min(TRELLIS_SETUP_WRITES);
            (first..end).try_for_each(|index| write_setup(transport, index))
        } else if part - setup_parts < TRELLIS_LED_CHUNKS {
            write_led_buffer_chunk(transport, &self.pixels[dev_idx], part - setup_parts)
        } else {
            write_led_show(transport).map(|()| self.delay.sleep(TRELLIS_SHOW_SETTLE))
        };
        result.inspect_err(|_| self.needs_setup[dev_idx] = true)?;
        Ok(part < setup_parts + TRELLIS_LED_CHUNKS)
    }

    /// Write an 8x8 RGB frame to the NeoPixel buffers of the boards whose pixels changed, then
    /// latch those. Every latch makes a board stretch the clock, so unchanged boards are left alone.
    pub fn write_led_frame(&mut self, frame: &[[u8; 3]; 64]) -> Result<(), String> {
        let mut changed = [false; 4];
        for (dev_idx, changed) in changed.iter_mut().enumerate() {
            let data = board_pixels(frame, dev_idx);
            if self.shown[dev_idx] && !self.needs_setup[dev_idx] && self.pixels[dev_idx] == data {
                continue;
            }
            self.shown[dev_idx] = false;
            (0..TRELLIS_LED_CHUNKS)
                .try_for_each(|chunk| {
                    write_led_buffer_chunk(&mut *self.transports[dev_idx], &data, chunk)
                })
                .inspect_err(|_| self.needs_setup[dev_idx] = true)?;
            self.pixels[dev_idx] = data;
            *changed = true;
        }
        for (dev_idx, transport) in self.transports.iter_mut().enumerate() {
            if changed[dev_idx] {
                write_led_show(&mut **transport)
                    .inspect_err(|_| self.needs_setup[dev_idx] = true)?;
                self.shown[dev_idx] = true;
            }
        }
        if changed.contains(&true) {
            self.delay.sleep(TRELLIS_SHOW_SETTLE);
        }
        Ok(())
    }
}

fn board_pixels(frame: &[[u8; 3]; 64], dev_idx: usize) -> [u8; TRELLIS_PIXEL_BYTES_PER_DEVICE] {
    let base_x = (dev_idx % 2) * 4;
    let base_y = (dev_idx / 2) * 4;
    let mut data = [0_u8; TRELLIS_PIXEL_BYTES_PER_DEVICE];
    for (index, pixel) in data.as_chunks_mut::<3>().0.iter_mut().enumerate() {
        *pixel = grb_color(frame[(base_y + index / 4) * 8 + base_x + index % 4]);
    }
    data
}

/// A Seesaw refreshing its 16 NeoPixels (about 30 µs each plus the latch gap) answers I2C late and
/// stretches the clock; the Raspberry's I2C controller mishandles stretching and can corrupt the
/// transfer and hang the chip until power-off. Only the addressed chip stretches, so the other
/// boards' SHOWs may follow at once; the bus then stays idle until every refresh is done.
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

fn grb_color(rgb: [u8; 3]) -> [u8; 3] {
    [rgb[1], rgb[0], rgb[2]]
}

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

fn write_led_buffer_chunk(
    transport: &mut dyn SeesawTransport,
    data: &[u8],
    chunk: usize,
) -> Result<(), String> {
    let offset = chunk * TRELLIS_LED_CHUNK_BYTES;
    let end = (offset + TRELLIS_LED_CHUNK_BYTES).min(data.len());
    let mut payload = Vec::with_capacity(2 + end - offset);
    payload.extend_from_slice(&(offset as u16).to_be_bytes());
    payload.extend_from_slice(&data[offset..end]);
    write_register(
        transport,
        SEESAW_NEOPIXEL_BASE,
        SEESAW_NEOPIXEL_BUF,
        &payload,
        "Trellis LED buffer write failed",
    )
}

fn write_led_show(transport: &mut dyn SeesawTransport) -> Result<(), String> {
    write_register(
        transport,
        SEESAW_NEOPIXEL_BASE,
        SEESAW_NEOPIXEL_SHOW,
        &[],
        "Trellis LED show failed",
    )
}

fn trellis_key_to_seesaw_key(key: u8) -> u8 {
    ((key / 4) * 8) + (key % 4)
}

fn seesaw_key_to_trellis_key(key: u8) -> u8 {
    ((key / 8) * 4) + (key % 8)
}

mod setup;
use setup::{configure_device, write_setup, SETUP_WRITES_PER_PART, TRELLIS_SETUP_WRITES};

#[cfg(test)]
mod tests;
