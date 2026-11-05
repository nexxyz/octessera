use super::{rotate_clockwise_rgb565, rotated_row_window, FRAME_BYTES};
use rppal::gpio::{Gpio, OutputPin};
use spidev::Spidev;
use std::io::Write;
use std::ops::Range;

/// SSD1351 commands
const CMD_SET_COLUMN_ADDR: u8 = 0x15;
const CMD_SET_ROW_ADDR: u8 = 0x75;
const CMD_WRITE_RAM: u8 = 0x5C;
const CMD_DISPLAY_ON: u8 = 0xAF;
const CMD_DISPLAY_OFF: u8 = 0xAE;
const CMD_NORMAL_DISPLAY: u8 = 0xA6;
const CMD_SET_REMAP: u8 = 0xA0;
const CMD_SET_START_LINE: u8 = 0xA1;
const CMD_SET_DISPLAY_OFFSET: u8 = 0xA2;
const CMD_SET_GPIO: u8 = 0xB5;
const CMD_FUNCTION_SELECTION: u8 = 0xAB;
const CMD_SET_PRECHARGE1: u8 = 0xB1;
const CMD_SET_CLOCK_DIV: u8 = 0xB3;
const CMD_SET_VSL: u8 = 0xB4;
const CMD_SET_PRECHARGE2: u8 = 0xB6;
const CMD_SET_PRECHARGE_VOLTAGE: u8 = 0xBB;
const CMD_SET_VCOMH: u8 = 0xBE;
const CMD_SET_CONTRAST: u8 = 0xC1;
const CMD_MASTER_CONTRAST: u8 = 0xC7;
const CMD_SET_MUX_RATIO: u8 = 0xCA;
const CMD_SET_COMMAND_LOCK: u8 = 0xFD;
const SPI_CHUNK_BYTES: usize = 4096;

/// OLED display driver
pub struct OledSsd1351 {
    spi: Spidev,
    dc: OutputPin,
    _rst: OutputPin,
    rotated_frame: Vec<u8>,
    window: Vec<u8>,
}

impl OledSsd1351 {
    /// Initialize OLED on SPI bus 0
    pub fn new() -> Result<Self, String> {
        Self::open(false)
    }

    pub fn adopt_existing() -> Result<Self, String> {
        Self::open(true)
    }

    pub fn detach_preserving(&mut self) -> Result<(), String> {
        Ok(())
    }

    pub fn reacquire_existing(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn open(preserve_existing: bool) -> Result<Self, String> {
        let startup_plan = crate::oled_startup_plan::OledStartupPlan::new(preserve_existing);
        // Open SPI device
        let mut spi =
            Spidev::open("/dev/spidev0.0").map_err(|e| format!("SPI open failed: {}", e))?;

        // Configure SPI: mode 0, 8-bit, 16MHz for the Adafruit SSD1351 breakout.
        let mut config = spidev::SpidevOptions::new();
        config.mode(spidev::SpiModeFlags::SPI_MODE_0);
        config.max_speed_hz(16_000_000);
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
            .into_output_high();

        if !startup_plan.operations().is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(250));

            // Hardware reset pulse
            rst.set_high();
            std::thread::sleep(std::time::Duration::from_millis(100));
            rst.set_low();
            std::thread::sleep(std::time::Duration::from_millis(100));
            rst.set_high();
            std::thread::sleep(std::time::Duration::from_millis(250));

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
            Self::write_command(&mut spi, &mut dc, CMD_SET_PRECHARGE_VOLTAGE, &[0x17])?;
            Self::write_command(&mut spi, &mut dc, CMD_SET_VCOMH, &[0x05])?;
            Self::write_command(&mut spi, &mut dc, CMD_NORMAL_DISPLAY, &[])?;
            Self::write_command(&mut spi, &mut dc, CMD_SET_CONTRAST, &[0xC8, 0x80, 0xC8])?;
            Self::write_command(&mut spi, &mut dc, CMD_MASTER_CONTRAST, &[0x0F])?;
            Self::write_command(&mut spi, &mut dc, CMD_SET_VSL, &[0xA0, 0xB5, 0x55])?;
            Self::write_command(&mut spi, &mut dc, CMD_SET_PRECHARGE2, &[0x01])?;

            Self::write_command(&mut spi, &mut dc, CMD_DISPLAY_ON, &[])?;
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        Ok(Self {
            spi,
            dc,
            _rst: rst,
            rotated_frame: vec![0_u8; FRAME_BYTES],
            window: Vec::with_capacity(FRAME_BYTES),
        })
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
        let frame = rotate_clockwise_rgb565(pixels, &mut self.rotated_frame);
        write_all_chunked(&mut self.spi, frame)
            .map_err(|e| format!("SPI frame write failed: {}", e))?;

        Ok(())
    }

    /// Writes only logical rows `rows` of a pre-rendered frame, as one RAM window.
    pub fn write_rows(&mut self, pixels: &[u8], rows: Range<usize>) -> Result<(), String> {
        let columns = rotated_row_window(pixels, rows, &mut self.window);
        Self::write_command(&mut self.spi, &mut self.dc, CMD_SET_COLUMN_ADDR, &columns)?;
        Self::write_command(&mut self.spi, &mut self.dc, CMD_SET_ROW_ADDR, &[0x00, 0x7F])?;
        Self::write_command(&mut self.spi, &mut self.dc, CMD_WRITE_RAM, &[])?;
        self.dc.set_high();
        write_all_chunked(&mut self.spi, &self.window)
            .map_err(|e| format!("SPI window write failed: {}", e))
    }

    pub fn display_on(&mut self) -> Result<(), String> {
        Self::write_command(&mut self.spi, &mut self.dc, CMD_NORMAL_DISPLAY, &[])?;
        Self::write_command(&mut self.spi, &mut self.dc, CMD_DISPLAY_ON, &[])
    }

    pub fn display_off(&mut self) -> Result<(), String> {
        Self::write_command(&mut self.spi, &mut self.dc, CMD_DISPLAY_OFF, &[])
    }

    /// Sets the master contrast current step (C7h), 0..=15.
    pub fn set_master_contrast(&mut self, step: u8) -> Result<(), String> {
        Self::write_command(
            &mut self.spi,
            &mut self.dc,
            CMD_MASTER_CONTRAST,
            &[step.min(0x0F)],
        )
    }
}

fn write_all_chunked(spi: &mut Spidev, data: &[u8]) -> std::io::Result<()> {
    for chunk in data.chunks(SPI_CHUNK_BYTES) {
        spi.write_all(chunk)?;
    }
    Ok(())
}
