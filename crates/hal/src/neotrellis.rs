//! NeoTrellis 8x8 LED matrix driver (4x4 devices x4 chain).

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
use crate::board_profiles::SeesawInputMode;
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
        Self::new_with_mode(i2c_path, TRELLIS_ADDRS, SeesawInputMode::Interrupt)
    }

    #[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w"))]
    pub fn new_with_mode(
        i2c_path: &str,
        addresses: [u16; 4],
        mode: SeesawInputMode,
    ) -> Result<Self, String> {
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
            match initialize_once(addresses, &mut transports, mode, &mut *delay) {
                Ok(()) => {
                    return Ok(Self {
                        devices: addresses.map(|address| (address, [0; 16])),
                        transports,
                        delay,
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
        mode: SeesawInputMode,
    ) -> Result<Self, String> {
        Self::initialize_with_retries(addresses, transports, mode, Box::new(NoopSeesawDelay))
    }

    #[cfg(test)]
    fn initialize_with_retries(
        addresses: [u16; 4],
        mut transports: [Box<dyn SeesawTransport>; 4],
        mode: SeesawInputMode,
        mut delay: Box<dyn SeesawDelay>,
    ) -> Result<Self, String> {
        let mut last_error = None;
        for attempt in 1..=TRELLIS_INIT_ATTEMPTS {
            match initialize_once(addresses, &mut transports, mode, &mut *delay) {
                Ok(()) => {
                    return Ok(Self {
                        devices: addresses.map(|address| (address, [0; 16])),
                        transports,
                        delay,
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

    /// Scan all keys, returning `(x, y, pressed)` in the existing lower-left coordinate space.
    pub fn scan_keys(&mut self) -> Result<Vec<(usize, usize, bool)>, String> {
        let mut result = Vec::new();
        for (dev_idx, (_, _)) in self.devices.iter().enumerate() {
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
                continue;
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
                    result.push((x, y, pressed));
                }
            }
        }
        Ok(result)
    }

    /// Write an 8x8 RGB frame to the four NeoPixel buffers.
    pub fn write_led_frame(&mut self, frame: &[[u8; 3]; 64]) -> Result<(), String> {
        for (dev_idx, _) in self.devices.iter().enumerate() {
            let base_x = (dev_idx % 2) * 4;
            let base_y = (dev_idx / 2) * 4;
            let mut data = Vec::with_capacity(TRELLIS_PIXEL_BYTES_PER_DEVICE);
            for y in base_y..(base_y + 4) {
                for x in base_x..(base_x + 4) {
                    data.extend_from_slice(&grb_color(frame[y * 8 + x]));
                }
            }
            write_led_buffer_chunks(&mut *self.transports[dev_idx], &data)?;
            write_register(
                &mut *self.transports[dev_idx],
                SEESAW_NEOPIXEL_BASE,
                SEESAW_NEOPIXEL_SHOW,
                &[],
                "Trellis LED show failed",
            )?;
            self.delay.sleep(Duration::from_micros(300));
        }
        Ok(())
    }
}

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
    mode: SeesawInputMode,
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
        let mut hw_id = [0_u8; 1];
        read_register(
            &mut **transport,
            SEESAW_STATUS_BASE,
            SEESAW_HW_ID,
            &mut hw_id,
            "Trellis HW ID read failed",
        )?;
        if !matches!(hw_id[0], 0x55 | 0x84..=0x89) {
            return Err(format!(
                "Trellis HW ID invalid at {address:#04x}: {:#04x}",
                hw_id[0]
            ));
        }
        if mode == SeesawInputMode::Interrupt {
            write_register(
                &mut **transport,
                SEESAW_KEYPAD_BASE,
                SEESAW_KEYPAD_INTENSET,
                &[0x01],
                "Trellis keypad interrupt init failed",
            )?;
        }
        for key in 0..TRELLIS_PIXELS_PER_DEVICE as u8 {
            let seesaw_key = trellis_key_to_seesaw_key(key);
            for edge in [KEYPAD_EDGE_FALLING, KEYPAD_EDGE_RISING] {
                let state = 0x01 | (1 << (edge + 1));
                write_register(
                    &mut **transport,
                    SEESAW_KEYPAD_BASE,
                    SEESAW_KEYPAD_EVENT,
                    &[seesaw_key, state],
                    "Trellis keypad event init failed",
                )?;
            }
        }
        write_register(
            &mut **transport,
            SEESAW_NEOPIXEL_BASE,
            SEESAW_NEOPIXEL_PIN,
            &[TRELLIS_NEOPIXEL_PIN],
            "Trellis LED pin init failed",
        )?;
        write_register(
            &mut **transport,
            SEESAW_NEOPIXEL_BASE,
            SEESAW_NEOPIXEL_BUF_LENGTH,
            &(TRELLIS_PIXEL_BYTES_PER_DEVICE as u16).to_be_bytes(),
            "Trellis LED length init failed",
        )?;
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

    pub fn scan_keys(&mut self) -> Result<Vec<(usize, usize, bool)>, String> {
        Ok(Vec::new())
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

#[cfg(test)]
#[path = "neotrellis_tests.rs"]
mod tests;
