//! NeoTrellis 8x8 LED matrix driver (4x4 devices x4 chain)
//! Uses seesaw over I2C.

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
use crate::board_profiles::SeesawInputMode;
#[cfg(feature = "rpi-zero-2w")]
use crate::pinmap::TRELLIS_ADDRS;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
use std::fs::{File, OpenOptions};
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
use std::io::{Read, Write};
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
use std::thread;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
use std::time::Duration;

#[cfg(not(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w")))]
use std::fmt;

/// NeoTrellis device (4x4, daisy-chained to make 8x8)
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
pub struct NeoTrellis {
    i2c_path: String,
    devices: [(u16, [u8; 16]); 4],
}

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_STATUS_BASE: u8 = 0x00;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_HW_ID: u8 = 0x01;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_SW_RESET: u8 = 0x7F;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_KEYPAD_BASE: u8 = 0x10;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_KEYPAD_EVENT: u8 = 0x01;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_KEYPAD_INTENSET: u8 = 0x02;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_KEYPAD_COUNT: u8 = 0x04;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_KEYPAD_FIFO: u8 = 0x10;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_NEOPIXEL_BASE: u8 = 0x0E;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_NEOPIXEL_PIN: u8 = 0x01;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_NEOPIXEL_BUF_LENGTH: u8 = 0x03;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_NEOPIXEL_BUF: u8 = 0x04;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_NEOPIXEL_SHOW: u8 = 0x05;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const TRELLIS_NEOPIXEL_PIN: u8 = 3;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w", test))]
const TRELLIS_PIXELS_PER_DEVICE: usize = 16;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const TRELLIS_PIXEL_BYTES_PER_DEVICE: usize = TRELLIS_PIXELS_PER_DEVICE * 3;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const TRELLIS_LED_CHUNK_BYTES: usize = 24;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const TRELLIS_INIT_ATTEMPTS: usize = 3;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const TRELLIS_INIT_RETRY_DELAY: Duration = Duration::from_millis(250);
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w", test))]
const KEYPAD_EDGE_FALLING: u8 = 2;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w", test))]
const KEYPAD_EDGE_RISING: u8 = 3;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
#[derive(Clone, Debug, Eq, PartialEq)]
enum TrellisInitCommand {
    Write {
        base: u8,
        function: u8,
        data: Vec<u8>,
    },
    Read {
        base: u8,
        function: u8,
        bytes: usize,
    },
}

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
fn trellis_reset_command() -> TrellisInitCommand {
    TrellisInitCommand::Write {
        base: SEESAW_STATUS_BASE,
        function: SEESAW_SW_RESET,
        data: vec![0xFF],
    }
}

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
fn trellis_init_plan(mode: SeesawInputMode) -> Vec<TrellisInitCommand> {
    let mut plan = vec![TrellisInitCommand::Read {
        base: SEESAW_STATUS_BASE,
        function: SEESAW_HW_ID,
        bytes: 1,
    }];
    if mode == SeesawInputMode::Interrupt {
        plan.push(TrellisInitCommand::Write {
            base: SEESAW_KEYPAD_BASE,
            function: SEESAW_KEYPAD_INTENSET,
            data: vec![0x01],
        });
    }
    for key in 0..TRELLIS_PIXELS_PER_DEVICE as u8 {
        let seesaw_key = trellis_key_to_seesaw_key(key);
        for edge in [KEYPAD_EDGE_FALLING, KEYPAD_EDGE_RISING] {
            let state = 0x01 | (1 << (edge + 1));
            plan.push(TrellisInitCommand::Write {
                base: SEESAW_KEYPAD_BASE,
                function: SEESAW_KEYPAD_EVENT,
                data: vec![seesaw_key, state],
            });
        }
    }
    plan.extend([
        TrellisInitCommand::Write {
            base: SEESAW_NEOPIXEL_BASE,
            function: SEESAW_NEOPIXEL_PIN,
            data: vec![TRELLIS_NEOPIXEL_PIN],
        },
        TrellisInitCommand::Write {
            base: SEESAW_NEOPIXEL_BASE,
            function: SEESAW_NEOPIXEL_BUF_LENGTH,
            data: (TRELLIS_PIXEL_BYTES_PER_DEVICE as u16)
                .to_be_bytes()
                .to_vec(),
        },
    ]);
    plan
}

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
impl NeoTrellis {
    #[cfg(feature = "rpi-zero-2w")]
    pub fn new(i2c_path: &str) -> Result<Self, String> {
        Self::new_with_mode(i2c_path, TRELLIS_ADDRS, SeesawInputMode::Interrupt)
    }

    pub fn new_with_mode(
        i2c_path: &str,
        addresses: [u16; 4],
        mode: SeesawInputMode,
    ) -> Result<Self, String> {
        let mut last_error = None;
        for attempt in 1..=TRELLIS_INIT_ATTEMPTS {
            match Self::try_new(i2c_path, addresses, mode) {
                Ok(trellis) => return Ok(trellis),
                Err(error) => {
                    last_error = Some(error);
                    if attempt < TRELLIS_INIT_ATTEMPTS {
                        thread::sleep(TRELLIS_INIT_RETRY_DELAY);
                    }
                }
            }
        }
        Err(last_error.unwrap_or_else(|| "Trellis init failed".to_string()))
    }

    fn try_new(i2c_path: &str, addresses: [u16; 4], mode: SeesawInputMode) -> Result<Self, String> {
        let devices = addresses.map(|addr| (addr, [0; 16]));

        let trellis = Self {
            i2c_path: i2c_path.to_string(),
            devices,
        };

        // Reset seesaw on each device before probing.
        let reset = trellis_reset_command();
        let TrellisInitCommand::Write {
            base,
            function,
            data,
        } = reset
        else {
            unreachable!();
        };
        for (addr, _) in &trellis.devices {
            let mut file = open_device(&trellis.i2c_path, *addr)?;
            write_register(&mut file, base, function, &data, "Trellis reset failed")?;
        }
        thread::sleep(Duration::from_millis(500));

        // Probe each device and configure its NeoPixel buffer length.
        for (addr, _) in &trellis.devices {
            let mut file = open_device(&trellis.i2c_path, *addr)?;
            for command in trellis_init_plan(mode) {
                match command {
                    TrellisInitCommand::Read {
                        base,
                        function,
                        bytes,
                    } => {
                        let mut id = [0_u8; 1];
                        read_register(
                            &mut file,
                            base,
                            function,
                            &mut id[..bytes],
                            "Trellis HW ID read failed",
                        )?;
                        if !matches!(id[0], 0x55 | 0x84..=0x89) {
                            return Err(format!(
                                "Trellis HW ID invalid at {:#04x}: {:#04x}",
                                addr, id[0]
                            ));
                        }
                    }
                    TrellisInitCommand::Write {
                        base,
                        function,
                        data,
                    } => {
                        let context = match (base, function) {
                            (SEESAW_KEYPAD_BASE, SEESAW_KEYPAD_INTENSET) => {
                                "Trellis keypad interrupt init failed"
                            }
                            (SEESAW_KEYPAD_BASE, SEESAW_KEYPAD_EVENT) => {
                                "Trellis keypad event init failed"
                            }
                            (SEESAW_NEOPIXEL_BASE, SEESAW_NEOPIXEL_PIN) => {
                                "Trellis LED pin init failed"
                            }
                            (SEESAW_NEOPIXEL_BASE, SEESAW_NEOPIXEL_BUF_LENGTH) => {
                                "Trellis LED length init failed"
                            }
                            _ => {
                                return Err(format!(
                                    "unknown Trellis init command: {base:#04x}/{function:#04x}"
                                ))
                            }
                        };
                        write_register(&mut file, base, function, &data, context)?;
                    }
                }
            }
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

            for event in buf.iter().take(key_count) {
                let Some((key_num, pressed)) = decode_trellis_key_event(*event) else {
                    continue;
                };
                if let Some((x, y)) = trellis_coordinate(dev_idx, key_num) {
                    result.push((x, y, pressed));
                }
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
                    let idx = y * 8 + x;
                    let rgb = &frame[idx];
                    data.extend_from_slice(&grb_color(*rgb));
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

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w", test))]
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

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w", test))]
fn grb_color(rgb: [u8; 3]) -> [u8; 3] {
    [rgb[1], rgb[0], rgb[2]]
}

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w", test))]
fn decode_trellis_key_event(key_data: u8) -> Option<(u8, bool)> {
    let edge = key_data & 0x03;
    if !matches!(edge, KEYPAD_EDGE_FALLING | KEYPAD_EDGE_RISING) {
        return None;
    }
    let pressed = edge == KEYPAD_EDGE_RISING;
    Some((seesaw_key_to_trellis_key(key_data >> 2), pressed))
}

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
fn open_device(i2c_path: &str, addr: u16) -> Result<File, String> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(i2c_path)
        .map_err(|e| format!("Trellis I2C open failed at {:#04x}: {}", addr, e))?;
    set_slave_addr(&file, addr)?;
    Ok(file)
}

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
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

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
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

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
fn trellis_key_to_seesaw_key(key: u8) -> u8 {
    ((key / 4) * 8) + (key % 4)
}

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w", test))]
fn seesaw_key_to_trellis_key(key: u8) -> u8 {
    ((key / 8) * 4) + (key % 8)
}

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
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

#[cfg(all(feature = "orange-pi-zero-2w", not(unix)))]
fn set_slave_addr(file: &File, addr: u16) -> Result<(), String> {
    let _ = (file, addr);
    Err("Orange Seesaw I2C requires a Unix target".into())
}

#[cfg(any(feature = "rpi-zero-2w", all(feature = "orange-pi-zero-2w", unix)))]
fn set_slave_addr(file: &File, addr: u16) -> Result<(), String> {
    #[cfg(all(unix, target_os = "linux"))]
    {
        use std::os::unix::io::AsRawFd;
        let result = unsafe { libc::ioctl(file.as_raw_fd(), 0x0703, addr as u64) };
        if result < 0 {
            return Err(format!(
                "I2C slave select failed for {addr:#04x}: {}",
                std::io::Error::last_os_error()
            ));
        }
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    let _ = (file, addr);
    Ok(())
}

#[cfg(not(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w")))]
pub struct NeoTrellis {
    _private: (),
}

#[cfg(not(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w")))]
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

#[cfg(not(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w")))]
impl fmt::Debug for NeoTrellis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NeoTrellis {{ ... }}")
    }
}

#[cfg(test)]
#[path = "neotrellis_tests.rs"]
mod tests;
