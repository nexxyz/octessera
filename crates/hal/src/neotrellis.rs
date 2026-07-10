//! NeoTrellis 8x8 LED matrix driver (4x4 devices x4 chain)
//! Uses seesaw over I2C.

#[cfg(feature = "rpi-zero-2w")]
use crate::pinmap::TRELLIS_ADDRS;
#[cfg(feature = "rpi-zero-2w")]
use std::fs::{File, OpenOptions};
#[cfg(feature = "rpi-zero-2w")]
use std::io::{Read, Write};
#[cfg(feature = "rpi-zero-2w")]
use std::os::unix::io::AsRawFd;
#[cfg(feature = "rpi-zero-2w")]
use std::thread;
#[cfg(feature = "rpi-zero-2w")]
use std::time::Duration;

#[cfg(not(feature = "rpi-zero-2w"))]
use std::fmt;

/// NeoTrellis device (4x4, daisy-chained to make 8x8)
#[cfg(feature = "rpi-zero-2w")]
pub struct NeoTrellis {
    i2c_path: String,
    devices: [(u16, [u8; 16]); 4],
}

#[cfg(feature = "rpi-zero-2w")]
const SEESAW_STATUS_BASE: u8 = 0x00;
#[cfg(feature = "rpi-zero-2w")]
const SEESAW_HW_ID: u8 = 0x01;
#[cfg(feature = "rpi-zero-2w")]
const SEESAW_SW_RESET: u8 = 0x7F;
#[cfg(feature = "rpi-zero-2w")]
const SEESAW_KEYPAD_BASE: u8 = 0x10;
#[cfg(feature = "rpi-zero-2w")]
const SEESAW_KEYPAD_EVENT: u8 = 0x01;
#[cfg(feature = "rpi-zero-2w")]
const SEESAW_KEYPAD_INTENSET: u8 = 0x02;
#[cfg(feature = "rpi-zero-2w")]
const SEESAW_KEYPAD_COUNT: u8 = 0x04;
#[cfg(feature = "rpi-zero-2w")]
const SEESAW_KEYPAD_FIFO: u8 = 0x10;
#[cfg(feature = "rpi-zero-2w")]
const SEESAW_NEOPIXEL_BASE: u8 = 0x0E;
#[cfg(feature = "rpi-zero-2w")]
const SEESAW_NEOPIXEL_PIN: u8 = 0x01;
#[cfg(feature = "rpi-zero-2w")]
const SEESAW_NEOPIXEL_BUF_LENGTH: u8 = 0x03;
#[cfg(feature = "rpi-zero-2w")]
const SEESAW_NEOPIXEL_BUF: u8 = 0x04;
#[cfg(feature = "rpi-zero-2w")]
const SEESAW_NEOPIXEL_SHOW: u8 = 0x05;
#[cfg(feature = "rpi-zero-2w")]
const TRELLIS_NEOPIXEL_PIN: u8 = 3;
#[cfg(feature = "rpi-zero-2w")]
const TRELLIS_PIXELS_PER_DEVICE: usize = 16;
#[cfg(feature = "rpi-zero-2w")]
const TRELLIS_PIXEL_BYTES_PER_DEVICE: usize = TRELLIS_PIXELS_PER_DEVICE * 3;
#[cfg(feature = "rpi-zero-2w")]
const TRELLIS_LED_CHUNK_BYTES: usize = 24;
#[cfg(any(feature = "rpi-zero-2w", test))]
const KEYPAD_EDGE_FALLING: u8 = 2;
#[cfg(any(feature = "rpi-zero-2w", test))]
const KEYPAD_EDGE_RISING: u8 = 3;

#[cfg(feature = "rpi-zero-2w")]
impl NeoTrellis {
    /// Initialize 4 NeoTrellis devices at the configured addresses.
    pub fn new(i2c_path: &str) -> Result<Self, String> {
        let devices = TRELLIS_ADDRS.map(|addr| (addr, [0; 16]));

        let trellis = Self {
            i2c_path: i2c_path.to_string(),
            devices,
        };

        // Reset seesaw on each device before probing.
        for (addr, _) in &trellis.devices {
            let mut file = open_device(&trellis.i2c_path, *addr)?;
            write_register(
                &mut file,
                SEESAW_STATUS_BASE,
                SEESAW_SW_RESET,
                &[0xFF],
                "Trellis reset failed",
            )?;
        }
        thread::sleep(Duration::from_millis(500));

        // Probe each device and configure its NeoPixel buffer length.
        for (addr, _) in &trellis.devices {
            let mut file = open_device(&trellis.i2c_path, *addr)?;
            let mut id = [0_u8; 1];
            read_register(
                &mut file,
                SEESAW_STATUS_BASE,
                SEESAW_HW_ID,
                &mut id,
                "Trellis HW ID read failed",
            )?;
            if !matches!(id[0], 0x55 | 0x84..=0x89) {
                return Err(format!(
                    "Trellis HW ID invalid at {:#04x}: {:#04x}",
                    addr, id[0]
                ));
            }

            write_register(
                &mut file,
                SEESAW_KEYPAD_BASE,
                SEESAW_KEYPAD_INTENSET,
                &[0x01],
                "Trellis keypad interrupt init failed",
            )?;
            for key in 0..TRELLIS_PIXELS_PER_DEVICE as u8 {
                let seesaw_key = trellis_key_to_seesaw_key(key);
                enable_keypad_event(&mut file, seesaw_key, KEYPAD_EDGE_FALLING)?;
                enable_keypad_event(&mut file, seesaw_key, KEYPAD_EDGE_RISING)?;
            }

            write_register(
                &mut file,
                SEESAW_NEOPIXEL_BASE,
                SEESAW_NEOPIXEL_PIN,
                &[TRELLIS_NEOPIXEL_PIN],
                "Trellis LED pin init failed",
            )?;
            let length = (TRELLIS_PIXEL_BYTES_PER_DEVICE as u16).to_be_bytes();
            write_register(
                &mut file,
                SEESAW_NEOPIXEL_BASE,
                SEESAW_NEOPIXEL_BUF_LENGTH,
                &length,
                "Trellis LED length init failed",
            )?;
        }

        Ok(trellis)
    }

    /// Scan all keys, return Vec<(x, y, pressed)>
    pub fn scan_keys(&mut self) -> Result<Vec<(usize, usize, bool)>, String> {
        let mut result = Vec::new();

        for (dev_idx, (addr, _)) in self.devices.iter().enumerate() {
            let mut file = open_device(&self.i2c_path, *addr)?;
            let mut count = [0_u8; 1];
            read_register(
                &mut file,
                SEESAW_KEYPAD_BASE,
                SEESAW_KEYPAD_COUNT,
                &mut count,
                "Trellis scan count failed",
            )?;

            let raw_count = usize::from(count[0]);
            if raw_count == 0 {
                continue;
            }
            let key_count = raw_count.min(16);

            let mut buf = [0_u8; 16];
            read_register(
                &mut file,
                SEESAW_KEYPAD_BASE,
                SEESAW_KEYPAD_FIFO,
                &mut buf[..key_count],
                "Trellis scan FIFO failed",
            )?;

            for i in 0..key_count {
                let Some((key_num, pressed)) = decode_trellis_key_event(buf[i]) else {
                    continue;
                };
                if key_num >= TRELLIS_PIXELS_PER_DEVICE as u8 {
                    continue;
                }
                // Map 4x4 key to 8x8 grid position
                let local_x = (key_num % 4) as usize;
                let local_y = (key_num / 4) as usize;
                let base_x = (dev_idx % 2) * 4;
                let base_y = (dev_idx / 2) * 4;
                let x = base_x + local_x;
                let y = 7 - (base_y + local_y);
                result.push((x, y, pressed));
            }
        }

        Ok(result)
    }

    /// Write LED frame (8x8 RGB values)
    pub fn write_led_frame(&mut self, frame: &[[u8; 3]; 64]) -> Result<(), String> {
        for (dev_idx, (addr, _)) in self.devices.iter().enumerate() {
            let base_x = (dev_idx % 2) * 4;
            let base_y = (dev_idx / 2) * 4;

            let mut file = open_device(&self.i2c_path, *addr)?;
            let mut data = Vec::with_capacity(TRELLIS_PIXEL_BYTES_PER_DEVICE);

            for y in base_y..(base_y + 4) {
                for x in base_x..(base_x + 4) {
                    let idx = (y * 8 + x) as usize;
                    let rgb = &frame[idx];
                    data.extend_from_slice(&[rgb[1], rgb[0], rgb[2]]);
                }
            }

            write_led_buffer_chunks(&mut file, &data)?;
            write_register(
                &mut file,
                SEESAW_NEOPIXEL_BASE,
                SEESAW_NEOPIXEL_SHOW,
                &[],
                "Trellis LED show failed",
            )?;
            thread::sleep(Duration::from_micros(300));
        }

        Ok(())
    }
}

#[cfg(any(feature = "rpi-zero-2w", test))]
fn decode_trellis_key_event(key_data: u8) -> Option<(u8, bool)> {
    let edge = key_data & 0x03;
    if !matches!(edge, KEYPAD_EDGE_FALLING | KEYPAD_EDGE_RISING) {
        return None;
    }
    let pressed = edge == KEYPAD_EDGE_RISING;
    Some((seesaw_key_to_trellis_key(key_data >> 2), pressed))
}

#[cfg(feature = "rpi-zero-2w")]
fn open_device(i2c_path: &str, addr: u16) -> Result<File, String> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(i2c_path)
        .map_err(|e| format!("Trellis I2C open failed at {:#04x}: {}", addr, e))?;
    set_slave_addr(&file, addr)?;
    Ok(file)
}

#[cfg(feature = "rpi-zero-2w")]
fn write_register(
    file: &mut File,
    base: u8,
    function: u8,
    data: &[u8],
    context: &str,
) -> Result<(), String> {
    let mut command = Vec::with_capacity(2 + data.len());
    command.push(base);
    command.push(function);
    command.extend_from_slice(data);
    file.write_all(&command)
        .map_err(|e| format!("{context}: {e}"))
}

#[cfg(feature = "rpi-zero-2w")]
fn write_led_buffer_chunks(file: &mut File, data: &[u8]) -> Result<(), String> {
    for offset in (0..data.len()).step_by(TRELLIS_LED_CHUNK_BYTES) {
        let end = (offset + TRELLIS_LED_CHUNK_BYTES).min(data.len());
        let mut chunk = Vec::with_capacity(2 + end - offset);
        chunk.extend_from_slice(&(offset as u16).to_be_bytes());
        chunk.extend_from_slice(&data[offset..end]);
        write_register(
            file,
            SEESAW_NEOPIXEL_BASE,
            SEESAW_NEOPIXEL_BUF,
            &chunk,
            "Trellis LED buffer write failed",
        )?;
    }
    Ok(())
}

#[cfg(feature = "rpi-zero-2w")]
fn enable_keypad_event(file: &mut File, key: u8, edge: u8) -> Result<(), String> {
    let state = 0x01 | (1 << (edge + 1));
    write_register(
        file,
        SEESAW_KEYPAD_BASE,
        SEESAW_KEYPAD_EVENT,
        &[key, state],
        "Trellis keypad event init failed",
    )
}

#[cfg(feature = "rpi-zero-2w")]
fn trellis_key_to_seesaw_key(key: u8) -> u8 {
    ((key / 4) * 8) + (key % 4)
}

#[cfg(any(feature = "rpi-zero-2w", test))]
fn seesaw_key_to_trellis_key(key: u8) -> u8 {
    ((key / 8) * 4) + (key % 8)
}

#[cfg(feature = "rpi-zero-2w")]
fn read_register(
    file: &mut File,
    base: u8,
    function: u8,
    buffer: &mut [u8],
    context: &str,
) -> Result<(), String> {
    file.write_all(&[base, function])
        .map_err(|e| format!("{context}: {e}"))?;
    thread::sleep(Duration::from_millis(1));
    file.read_exact(buffer)
        .map_err(|e| format!("{context}: {e}"))
}

#[cfg(feature = "rpi-zero-2w")]
fn set_slave_addr(file: &File, addr: u16) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    unsafe {
        let result = libc::ioctl(file.as_raw_fd(), 0x0703, addr as u64); // I2C_SLAVE = 0x0703
        if result < 0 {
            return Err(format!(
                "I2C slave select failed for {addr:#04x}: {}",
                std::io::Error::last_os_error()
            ));
        }
    }
    Ok(())
}

/// Stub for non-Pi builds
#[cfg(not(feature = "rpi-zero-2w"))]
pub struct NeoTrellis {
    _private: (),
}

#[cfg(not(feature = "rpi-zero-2w"))]
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

#[cfg(not(feature = "rpi-zero-2w"))]
impl fmt::Debug for NeoTrellis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NeoTrellis {{ ... }}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rising_edge_is_press_and_falling_edge_is_release() {
        let seesaw_key = 10;
        assert_eq!(
            decode_trellis_key_event((seesaw_key << 2) | KEYPAD_EDGE_RISING),
            Some((6, true))
        );
        assert_eq!(
            decode_trellis_key_event((seesaw_key << 2) | KEYPAD_EDGE_FALLING),
            Some((6, false))
        );
    }

    #[test]
    fn non_edge_events_are_ignored() {
        assert_eq!(decode_trellis_key_event(0), None);
        assert_eq!(decode_trellis_key_event(1), None);
    }
}
