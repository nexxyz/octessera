//! SSD1351 OLED driver (128x128, 16-bit color, SPI interface)
//! For Adafruit 1431 / generic SSD1351 breakout.

#[cfg(feature = "pi-zero")]
use rppal::gpio::{Gpio, OutputPin};
#[cfg(feature = "pi-zero")]
use spidev::Spidev;
#[cfg(feature = "pi-zero")]
use std::io::Write;

#[cfg(not(feature = "pi-zero"))]
use std::fmt;

/// SSD1351 commands
#[cfg(feature = "pi-zero")]
const CMD_SET_COLUMN_ADDR: u8 = 0x15;
#[cfg(feature = "pi-zero")]
const CMD_SET_ROW_ADDR: u8 = 0x75;
#[cfg(feature = "pi-zero")]
const CMD_WRITE_RAM: u8 = 0x5C;
#[cfg(feature = "pi-zero")]
const CMD_DISPLAY_ON: u8 = 0xAF;
#[cfg(feature = "pi-zero")]
const CMD_DISPLAY_OFF: u8 = 0xAE;
#[cfg(feature = "pi-zero")]
const CMD_NORMAL_DISPLAY: u8 = 0xA6;
#[cfg(feature = "pi-zero")]
const CMD_SET_REMAP: u8 = 0xA0;
#[cfg(feature = "pi-zero")]
const CMD_SET_START_LINE: u8 = 0xA1;
#[cfg(feature = "pi-zero")]
const CMD_SET_DISPLAY_OFFSET: u8 = 0xA2;
#[cfg(feature = "pi-zero")]
const CMD_SET_GPIO: u8 = 0xB5;
#[cfg(feature = "pi-zero")]
const CMD_FUNCTION_SELECTION: u8 = 0xAB;
#[cfg(feature = "pi-zero")]
const CMD_SET_PRECHARGE1: u8 = 0xB1;
#[cfg(feature = "pi-zero")]
const CMD_SET_CLOCK_DIV: u8 = 0xB3;
#[cfg(feature = "pi-zero")]
const CMD_SET_VSL: u8 = 0xB4;
#[cfg(feature = "pi-zero")]
const CMD_SET_PRECHARGE2: u8 = 0xB6;
#[cfg(feature = "pi-zero")]
const CMD_SET_VCOMH: u8 = 0xBE;
#[cfg(feature = "pi-zero")]
const CMD_SET_CONTRAST: u8 = 0xC1;
#[cfg(feature = "pi-zero")]
const CMD_MASTER_CONTRAST: u8 = 0xC7;
#[cfg(feature = "pi-zero")]
const CMD_SET_MUX_RATIO: u8 = 0xCA;
#[cfg(feature = "pi-zero")]
const CMD_SET_COMMAND_LOCK: u8 = 0xFD;
#[cfg(feature = "pi-zero")]
const SPI_CHUNK_BYTES: usize = 4096;

/// OLED display driver
#[cfg(feature = "pi-zero")]
pub struct OledSsd1351 {
    spi: Spidev,
    dc: OutputPin,
    _rst: OutputPin,
}

#[cfg(feature = "pi-zero")]
impl OledSsd1351 {
    /// Initialize OLED on SPI bus 0
    pub fn new() -> Result<Self, String> {
        // Open SPI device
        let mut spi =
            Spidev::open("/dev/spidev0.0").map_err(|e| format!("SPI open failed: {}", e))?;

        // Configure SPI: mode 0, 8-bit, 1MHz for reliable bring-up.
        let mut config = spidev::SpidevOptions::new();
        config.mode(spidev::SpiModeFlags::SPI_MODE_0);
        config.max_speed_hz(1_000_000u32);
        config.bits_per_word(8);
        spi.configure(&config)
            .map_err(|e| format!("SPI configure failed: {}", e))?;

        // Get GPIO handles
        let gpio = Gpio::new().map_err(|e| e.to_string())?;
        let mut dc = gpio
            .get(crate::pinmap::OLED_DC)
            .map_err(|e| e.to_string())?
            .into_output();
        let mut rst = gpio
            .get(crate::pinmap::OLED_RST)
            .map_err(|e| e.to_string())?
            .into_output();

        // Hardware reset pulse
        rst.set_low();
        std::thread::sleep(std::time::Duration::from_millis(100));
        rst.set_high();
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Init sequence for SSD1351 / Adafruit 1431.
        Self::write_command(&mut spi, &mut dc, CMD_SET_COMMAND_LOCK, &[0x12])?;
        Self::write_command(&mut spi, &mut dc, CMD_SET_COMMAND_LOCK, &[0xB1])?;
        Self::write_command(&mut spi, &mut dc, CMD_DISPLAY_OFF, &[])?;
        Self::write_command(&mut spi, &mut dc, CMD_SET_CLOCK_DIV, &[0xF1])?;

        Self::write_command(&mut spi, &mut dc, CMD_SET_MUX_RATIO, &[0x7F])?;
        Self::write_command(&mut spi, &mut dc, CMD_SET_REMAP, &[0x74])?;
        Self::write_command(&mut spi, &mut dc, CMD_SET_COLUMN_ADDR, &[0x00, 0x7F])?;
        Self::write_command(&mut spi, &mut dc, CMD_SET_ROW_ADDR, &[0x00, 0x7F])?;
        Self::write_command(&mut spi, &mut dc, CMD_SET_START_LINE, &[0x00])?;
        Self::write_command(&mut spi, &mut dc, CMD_SET_DISPLAY_OFFSET, &[0x00])?;

        Self::write_command(&mut spi, &mut dc, CMD_SET_GPIO, &[0x00])?;
        Self::write_command(&mut spi, &mut dc, CMD_FUNCTION_SELECTION, &[0x01])?;
        Self::write_command(&mut spi, &mut dc, CMD_SET_PRECHARGE1, &[0x32])?;
        Self::write_command(&mut spi, &mut dc, CMD_SET_VCOMH, &[0x05])?;
        Self::write_command(&mut spi, &mut dc, CMD_NORMAL_DISPLAY, &[])?;
        Self::write_command(&mut spi, &mut dc, CMD_SET_CONTRAST, &[0xC8, 0x80, 0xC8])?;
        Self::write_command(&mut spi, &mut dc, CMD_MASTER_CONTRAST, &[0x0F])?;
        Self::write_command(&mut spi, &mut dc, CMD_SET_VSL, &[0xA0, 0xB5, 0x55])?;
        Self::write_command(&mut spi, &mut dc, CMD_SET_PRECHARGE2, &[0x01])?;

        Self::write_command(&mut spi, &mut dc, CMD_DISPLAY_ON, &[])?;

        Ok(Self { spi, dc, _rst: rst })
    }

    /// Write command + optional data bytes
    fn write_command(
        spi: &mut Spidev,
        dc: &mut OutputPin,
        cmd: u8,
        data: &[u8],
    ) -> Result<(), String> {
        // DC low = command
        dc.set_low();
        spi.write_all(&[cmd])
            .map_err(|e| format!("SPI write failed: {}", e))?;

        if !data.is_empty() {
            // DC high = data
            dc.set_high();
            write_all_chunked(spi, data).map_err(|e| format!("SPI write failed: {}", e))?;
        }

        Ok(())
    }

    /// Write pre-rendered RGB565 frame (128x128x2 bytes)
    pub fn write_frame(&mut self, pixels: &[u8]) -> Result<(), String> {
        // Set column address: 0-127
        Self::write_command(
            &mut self.spi,
            &mut self.dc,
            CMD_SET_COLUMN_ADDR,
            &[0x00, 0x7F],
        )?;

        // Set row address: 0-127
        Self::write_command(&mut self.spi, &mut self.dc, CMD_SET_ROW_ADDR, &[0x00, 0x7F])?;

        // Write to RAM
        Self::write_command(&mut self.spi, &mut self.dc, CMD_WRITE_RAM, &[])?;
        self.dc.set_high();
        write_all_chunked(&mut self.spi, pixels)
            .map_err(|e| format!("SPI frame write failed: {}", e))?;

        Ok(())
    }
}

#[cfg(feature = "pi-zero")]
fn write_all_chunked(spi: &mut Spidev, data: &[u8]) -> std::io::Result<()> {
    for chunk in data.chunks(SPI_CHUNK_BYTES) {
        spi.write_all(chunk)?;
    }
    Ok(())
}

/// Stub for non-Pi builds
#[cfg(not(feature = "pi-zero"))]
pub struct OledSsd1351 {
    _private: (),
}

#[cfg(not(feature = "pi-zero"))]
impl OledSsd1351 {
    pub fn new() -> Result<Self, String> {
        Ok(Self { _private: () })
    }

    pub fn write_frame(&mut self, _pixels: &[u8]) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(not(feature = "pi-zero"))]
impl fmt::Debug for OledSsd1351 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OledSsd1351 {{ ... }}")
    }
}
