//! NeoKey 1x4 button + LED driver
//! Uses seesaw over I2C.

#[cfg(feature = "pi-zero")]
use crate::pinmap::NEOKEY_ADDR;
#[cfg(feature = "pi-zero")]
use std::fs::{File, OpenOptions};
#[cfg(feature = "pi-zero")]
use std::io::{Read, Write};
#[cfg(feature = "pi-zero")]
use std::os::unix::io::AsRawFd;
#[cfg(feature = "pi-zero")]
use std::thread;
#[cfg(feature = "pi-zero")]
use std::time::Duration;

#[cfg(not(feature = "pi-zero"))]
use std::fmt;

/// NeoKey 1x4 device
#[cfg(feature = "pi-zero")]
pub struct NeoKey {
    i2c_path: String,
    addr: u16,
    stable_buttons: [bool; 4],
}

#[cfg(feature = "pi-zero")]
const SEESAW_STATUS_BASE: u8 = 0x00;
#[cfg(feature = "pi-zero")]
const SEESAW_HW_ID: u8 = 0x01;
#[cfg(feature = "pi-zero")]
const SEESAW_SW_RESET: u8 = 0x7F;
#[cfg(feature = "pi-zero")]
const SEESAW_GPIO_BASE: u8 = 0x01;
#[cfg(feature = "pi-zero")]
const SEESAW_GPIO_DIRCLR_BULK: u8 = 0x03;
#[cfg(feature = "pi-zero")]
const SEESAW_GPIO_BULK: u8 = 0x04;
#[cfg(feature = "pi-zero")]
const SEESAW_GPIO_BULK_SET: u8 = 0x05;
#[cfg(feature = "pi-zero")]
const SEESAW_GPIO_INTENSET: u8 = 0x08;
#[cfg(feature = "pi-zero")]
const SEESAW_GPIO_INTFLAG: u8 = 0x0A;
#[cfg(feature = "pi-zero")]
const SEESAW_GPIO_PULLENSET: u8 = 0x0B;
#[cfg(feature = "pi-zero")]
const SEESAW_NEOPIXEL_BASE: u8 = 0x0E;
#[cfg(feature = "pi-zero")]
const SEESAW_NEOPIXEL_PIN: u8 = 0x01;
#[cfg(feature = "pi-zero")]
const SEESAW_NEOPIXEL_BUF_LENGTH: u8 = 0x03;
#[cfg(feature = "pi-zero")]
const SEESAW_NEOPIXEL_BUF: u8 = 0x04;
#[cfg(feature = "pi-zero")]
const SEESAW_NEOPIXEL_SHOW: u8 = 0x05;
#[cfg(feature = "pi-zero")]
const NEOKEY_BUTTON_MASK: u32 = 0xF0;
#[cfg(feature = "pi-zero")]
const NEOKEY_NEOPIXEL_PIN: u8 = 3;
#[cfg(feature = "pi-zero")]
const NEOKEY_LED_BYTES: u16 = 12;

#[cfg(feature = "pi-zero")]
impl NeoKey {
    /// Initialize NeoKey at the configured address.
    pub fn new(i2c_path: &str) -> Result<Self, String> {
        let mut file = open_device(i2c_path, NEOKEY_ADDR)?;
        write_register(
            &mut file,
            SEESAW_STATUS_BASE,
            SEESAW_SW_RESET,
            &[0xFF],
            "NeoKey reset failed",
        )?;
        thread::sleep(Duration::from_millis(500));

        let mut id = [0_u8; 1];
        read_register(
            &mut file,
            SEESAW_STATUS_BASE,
            SEESAW_HW_ID,
            &mut id,
            "NeoKey HW ID read failed",
        )?;
        if !matches!(id[0], 0x55 | 0x84..=0x89) {
            return Err(format!("NeoKey HW ID invalid: {:#04x}", id[0]));
        }

        let mask = NEOKEY_BUTTON_MASK.to_be_bytes();
        write_register(
            &mut file,
            SEESAW_GPIO_BASE,
            SEESAW_GPIO_DIRCLR_BULK,
            &mask,
            "NeoKey GPIO direction init failed",
        )?;
        write_register(
            &mut file,
            SEESAW_GPIO_BASE,
            SEESAW_GPIO_PULLENSET,
            &mask,
            "NeoKey GPIO pullup init failed",
        )?;
        write_register(
            &mut file,
            SEESAW_GPIO_BASE,
            SEESAW_GPIO_BULK_SET,
            &mask,
            "NeoKey GPIO pullup set failed",
        )?;
        write_register(
            &mut file,
            SEESAW_GPIO_BASE,
            SEESAW_GPIO_INTENSET,
            &mask,
            "NeoKey GPIO interrupt init failed",
        )?;
        let mut int_flags = [0_u8; 4];
        read_register(
            &mut file,
            SEESAW_GPIO_BASE,
            SEESAW_GPIO_INTFLAG,
            &mut int_flags,
            "NeoKey GPIO interrupt clear failed",
        )?;
        write_register(
            &mut file,
            SEESAW_NEOPIXEL_BASE,
            SEESAW_NEOPIXEL_PIN,
            &[NEOKEY_NEOPIXEL_PIN],
            "NeoKey LED pin init failed",
        )?;
        write_register(
            &mut file,
            SEESAW_NEOPIXEL_BASE,
            SEESAW_NEOPIXEL_BUF_LENGTH,
            &NEOKEY_LED_BYTES.to_be_bytes(),
            "NeoKey LED length init failed",
        )?;

        Ok(Self {
            i2c_path: i2c_path.to_string(),
            addr: NEOKEY_ADDR,
            stable_buttons: [false; 4],
        })
    }

    /// Returns Vec<(key_index, pressed)> for keys 0-3.
    pub fn scan(&mut self) -> Result<Vec<(u8, bool)>, String> {
        let sampled = neokey_buttons_from_raw(self.raw_button_state()?);
        if sampled != self.stable_buttons {
            thread::sleep(Duration::from_millis(1));
            let confirmed = neokey_buttons_from_raw(self.raw_button_state()?);
            for index in 0..4 {
                if sampled[index] == confirmed[index] {
                    self.stable_buttons[index] = sampled[index];
                }
            }
        }

        let mut result = Vec::new();
        for i in 0..4 {
            result.push((i, self.stable_buttons[usize::from(i)]));
        }

        Ok(result)
    }

    pub fn scan_interrupts(&mut self) -> Result<Vec<(u8, bool)>, String> {
        self.clear_interrupt_flags()?;
        self.scan()
    }

    pub fn raw_button_state(&mut self) -> Result<u32, String> {
        let mut file = open_device(&self.i2c_path, self.addr)?;
        let mut buf = [0_u8; 4];
        read_register(
            &mut file,
            SEESAW_GPIO_BASE,
            SEESAW_GPIO_BULK,
            &mut buf,
            "NeoKey raw scan failed",
        )?;
        Ok(u32::from_be_bytes(buf))
    }

    fn clear_interrupt_flags(&mut self) -> Result<(), String> {
        let mut file = open_device(&self.i2c_path, self.addr)?;
        let mut buf = [0_u8; 4];
        read_register(
            &mut file,
            SEESAW_GPIO_BASE,
            SEESAW_GPIO_INTFLAG,
            &mut buf,
            "NeoKey GPIO interrupt clear failed",
        )
    }

    /// Set LED color for key (0-3)
    pub fn set_led(&mut self, key: u8, r: u8, g: u8, b: u8) -> Result<(), String> {
        if key >= 4 {
            return Err(format!("NeoKey LED index out of range: {key}"));
        }
        let mut file = open_device(&self.i2c_path, self.addr)?;
        let offset = u16::from(key) * 3;
        let mut data = Vec::with_capacity(5);
        data.extend_from_slice(&offset.to_be_bytes());
        data.extend_from_slice(&[g, r, b]);
        write_register(
            &mut file,
            SEESAW_NEOPIXEL_BASE,
            SEESAW_NEOPIXEL_BUF,
            &data,
            "NeoKey LED write failed",
        )?;
        write_register(
            &mut file,
            SEESAW_NEOPIXEL_BASE,
            SEESAW_NEOPIXEL_SHOW,
            &[],
            "NeoKey LED show failed",
        )?;
        thread::sleep(Duration::from_micros(300));
        Ok(())
    }
}

#[cfg(feature = "pi-zero")]
fn neokey_buttons_from_raw(raw: u32) -> [bool; 4] {
    let state = raw & NEOKEY_BUTTON_MASK;
    [
        (state & (1 << 4)) == 0,
        (state & (1 << 5)) == 0,
        (state & (1 << 6)) == 0,
        (state & (1 << 7)) == 0,
    ]
}

#[cfg(feature = "pi-zero")]
fn open_device(i2c_path: &str, addr: u16) -> Result<File, String> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(i2c_path)
        .map_err(|e| format!("NeoKey I2C open failed at {addr:#04x}: {e}"))?;
    set_slave_addr(&file, addr)?;
    Ok(file)
}

#[cfg(feature = "pi-zero")]
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

#[cfg(feature = "pi-zero")]
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

#[cfg(feature = "pi-zero")]
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
#[cfg(not(feature = "pi-zero"))]
pub struct NeoKey {
    _private: (),
}

#[cfg(not(feature = "pi-zero"))]
impl NeoKey {
    pub fn new(_i2c_path: &str) -> Result<Self, String> {
        Ok(Self { _private: () })
    }

    pub fn scan(&mut self) -> Result<Vec<(u8, bool)>, String> {
        Ok(Vec::new())
    }

    pub fn scan_interrupts(&mut self) -> Result<Vec<(u8, bool)>, String> {
        self.scan()
    }

    pub fn raw_button_state(&mut self) -> Result<u32, String> {
        Ok(0)
    }

    pub fn set_led(&mut self, _key: u8, _r: u8, _g: u8, _b: u8) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(not(feature = "pi-zero"))]
impl fmt::Debug for NeoKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NeoKey {{ ... }}")
    }
}
