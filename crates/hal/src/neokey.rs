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

#[cfg(not(feature = "pi-zero"))]
use std::fmt;

/// NeoKey 1x4 device
#[cfg(feature = "pi-zero")]
pub struct NeoKey {
    i2c_path: String,
    addr: u16,
}

#[cfg(feature = "pi-zero")]
impl NeoKey {
    /// Initialize NeoKey at the configured address.
    pub fn new(i2c_path: &str) -> Result<Self, String> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(i2c_path)
            .map_err(|e| format!("NeoKey init failed: {}", e))?;

        // Set slave address (I2C_SLAVE = 0x0703)
        set_slave_addr(&file, NEOKEY_ADDR)?;

        // Init seesaw
        let init_cmd = [0xFE, 0x41]; // Seesaw HW_ID
        file.write_all(&init_cmd)
            .map_err(|e| format!("NeoKey init failed: {}", e))?;

        Ok(Self {
            i2c_path: i2c_path.to_string(),
            addr: NEOKEY_ADDR,
        })
    }

    /// Returns Vec<(key_index, pressed)> for keys 0-3.
    pub fn scan(&mut self) -> Result<Vec<(u8, bool)>, String> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.i2c_path)
            .map_err(|e| format!("NeoKey scan failed: {}", e))?;

        set_slave_addr(&file, self.addr)?;

        // Read keypad FIFO (seesaw register 0x10)
        let read_cmd = [0x10];
        file.write_all(&read_cmd)
            .map_err(|e| format!("NeoKey scan failed: {}", e))?;

        let mut buf = [0u8; 4];
        file.read_exact(&mut buf)
            .map_err(|e| format!("NeoKey scan failed: {}", e))?;

        let mut result = Vec::new();
        for i in 0..4 {
            let pressed = (buf[0] & (1 << i)) != 0;
            result.push((i, pressed));
        }

        Ok(result)
    }

    /// Set LED color for key (0-3)
    pub fn set_led(&mut self, key: u8, r: u8, g: u8, b: u8) -> Result<(), String> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.i2c_path)
            .map_err(|e| format!("NeoKey LED write failed: {}", e))?;

        set_slave_addr(&file, self.addr)?;

        let cmd = [0x0E, key, r, g, b]; // Seesaw LED set
        file.write_all(&cmd)
            .map_err(|e| format!("NeoKey LED write failed: {}", e))
    }
}

#[cfg(feature = "pi-zero")]
fn set_slave_addr(file: &File, addr: u16) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    unsafe {
        libc::ioctl(file.as_raw_fd(), 0x0703, addr as u64); // I2C_SLAVE = 0x0703
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
