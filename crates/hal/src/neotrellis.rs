//! NeoTrellis 8x8 LED matrix driver (4x4 devices x4 chain)
//! Uses seesaw over I2C.

#[cfg(feature = "pi-zero")]
use crate::pinmap::TRELLIS_ADDRS;
#[cfg(feature = "pi-zero")]
use std::fs::{File, OpenOptions};
#[cfg(feature = "pi-zero")]
use std::io::{Read, Write};
#[cfg(feature = "pi-zero")]
use std::os::unix::io::AsRawFd;

#[cfg(not(feature = "pi-zero"))]
use std::fmt;

/// NeoTrellis device (4x4, daisy-chained to make 8x8)
#[cfg(feature = "pi-zero")]
pub struct NeoTrellis {
    i2c_path: String,
    devices: [(u16, [u8; 16]); 4],
}

#[cfg(feature = "pi-zero")]
impl NeoTrellis {
    /// Initialize 4 NeoTrellis devices at the configured addresses.
    pub fn new(i2c_path: &str) -> Result<Self, String> {
        let devices = TRELLIS_ADDRS.map(|addr| (addr, [0; 16]));

        let trellis = Self {
            i2c_path: i2c_path.to_string(),
            devices,
        };

        // Initialize seesaw on each device
        for (addr, _) in &trellis.devices {
            // Seesaw init: set module base, enable keypad
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&trellis.i2c_path)
                .map_err(|e| format!("Trellis init failed at {:#04x}: {}", addr, e))?;

            // Set slave address
            set_slave_addr(&file, *addr)?;

            // Seesaw HW_ID check
            let init_cmd = [0xFE, 0x41];
            file.write_all(&init_cmd)
                .map_err(|e| format!("Trellis init failed: {}", e))?;
        }

        Ok(trellis)
    }

    /// Scan all keys, return Vec<(x, y, pressed)>
    pub fn scan_keys(&mut self) -> Result<Vec<(usize, usize, bool)>, String> {
        let mut result = Vec::new();

        for (dev_idx, (addr, _)) in self.devices.iter().enumerate() {
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&self.i2c_path)
                .map_err(|e| format!("Trellis scan failed: {}", e))?;

            set_slave_addr(&file, *addr)?;

            // Read keypad FIFO (seesaw register 0x10)
            let read_cmd = [0x10];
            file.write_all(&read_cmd)
                .map_err(|e| format!("Trellis scan failed: {}", e))?;

            let mut buf = [0u8; 4];
            file.read_exact(&mut buf)
                .map_err(|e| format!("Trellis scan failed: {}", e))?;

            let key_count = buf[0];
            for i in 0..key_count {
                let key_data = buf[i as usize + 1];
                let pressed = (key_data & 0x80) == 0;
                let key_num = key_data & 0x7F;
                // Map 4x4 key to 8x8 grid position
                let local_x = (key_num % 4) as usize;
                let local_y = (key_num / 4) as usize;
                let base_x = (dev_idx % 2) * 4;
                let base_y = (dev_idx / 2) * 4;
                let x = base_x + local_x;
                let y = base_y + local_y;
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

            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&self.i2c_path)
                .map_err(|e| format!("Trellis LED write failed: {}", e))?;

            set_slave_addr(&file, *addr)?;

            // Batch write LED states via seesaw
            let mut cmd = vec![0x0E, 0x00]; // Seesaw LED base.

            for y in base_y..(base_y + 4) {
                for x in base_x..(base_x + 4) {
                    let idx = (y * 8 + x) as usize;
                    let rgb = &frame[idx];
                    cmd.push(rgb[0]); // R
                    cmd.push(rgb[1]); // G
                    cmd.push(rgb[2]); // B
                }
            }

            file.write_all(&cmd)
                .map_err(|e| format!("Trellis LED write failed: {}", e))?;
        }

        Ok(())
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
pub struct NeoTrellis {
    _private: (),
}

#[cfg(not(feature = "pi-zero"))]
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

#[cfg(not(feature = "pi-zero"))]
impl fmt::Debug for NeoTrellis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NeoTrellis {{ ... }}")
    }
}
