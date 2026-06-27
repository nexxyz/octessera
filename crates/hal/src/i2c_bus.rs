//! Shared I2C bus manager for NeoTrellis + NeoKey (seesaw devices)

#[cfg(feature = "pi-zero")]
use std::fs::{File, OpenOptions};
#[cfg(feature = "pi-zero")]
use std::io::{Read, Write};
#[cfg(feature = "pi-zero")]
use std::os::unix::io::AsRawFd;

#[cfg(not(feature = "pi-zero"))]
use std::fmt;

/// I2C bus wrapper for Pi Zero 2W
#[cfg(feature = "pi-zero")]
pub struct I2CBus {
    bus_path: String,
}

#[cfg(feature = "pi-zero")]
impl I2CBus {
    /// Open I2C bus (e.g., bus=1 opens /dev/i2c-1)
    pub fn new(bus: u8) -> Result<Self, String> {
        let path = format!("/dev/i2c-{}", bus);
        File::open(&path).map_err(|e| format!("I2C open failed: {}", e))?;
        Ok(Self { bus_path: path })
    }

    /// Write + read in single I2C transaction
    pub fn write_read(&self, addr: u16, write: &[u8], read: &mut [u8]) -> Result<(), String> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.bus_path)
            .map_err(|e| format!("I2C open failed: {}", e))?;

        set_slave_addr(&file, addr)?;

        file.write_all(write)
            .map_err(|e| format!("I2C write failed: {}", e))?;
        file.read_exact(read)
            .map_err(|e| format!("I2C read failed: {}", e))?;

        Ok(())
    }

    /// Simple write to I2C device
    pub fn write(&self, addr: u16, data: &[u8]) -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .open(&self.bus_path)
            .map_err(|e| format!("I2C open failed: {}", e))?;

        set_slave_addr(&file, addr)?;

        file.write_all(data)
            .map_err(|e| format!("I2C write failed: {}", e))
    }

    /// Read from I2C device
    pub fn read(&self, addr: u16, data: &mut [u8]) -> Result<(), String> {
        let mut file = OpenOptions::new()
            .read(true)
            .open(&self.bus_path)
            .map_err(|e| format!("I2C open failed: {}", e))?;

        set_slave_addr(&file, addr)?;

        file.read_exact(data)
            .map_err(|e| format!("I2C read failed: {}", e))
    }
}

#[cfg(feature = "pi-zero")]
fn set_slave_addr(file: &File, addr: u16) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let result = unsafe { libc::ioctl(file.as_raw_fd(), 0x0703, addr as u64) }; // I2C_SLAVE = 0x0703
        if result < 0 {
            return Err(format!(
                "I2C set slave address 0x{addr:02x} failed: {}",
                std::io::Error::last_os_error()
            ));
        }
    }
    Ok(())
}

/// Stub for non-Pi builds
#[cfg(not(feature = "pi-zero"))]
pub struct I2CBus {
    _private: (),
}

#[cfg(not(feature = "pi-zero"))]
impl I2CBus {
    pub fn new(_bus: u8) -> Result<Self, String> {
        Ok(Self { _private: () })
    }
    pub fn write_read(&self, _addr: u16, _write: &[u8], _read: &mut [u8]) -> Result<(), String> {
        Ok(())
    }
    pub fn write(&self, _addr: u16, _data: &[u8]) -> Result<(), String> {
        Ok(())
    }
    pub fn read(&self, _addr: u16, _data: &mut [u8]) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(not(feature = "pi-zero"))]
impl fmt::Debug for I2CBus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "I2CBus {{ ... }}")
    }
}
