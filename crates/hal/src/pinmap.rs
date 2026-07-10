//! Pin mapping for octessera hardware on Pi Zero 2W

pub use crate::board_profiles::EncoderPins;

use crate::board_profiles::RPI_ZERO_2W;

pub const I2C_BUS: u8 = RPI_ZERO_2W.i2c_bus;
pub const I2C_PATH: &str = RPI_ZERO_2W.i2c_path;

pub const SPI_BUS: &str = RPI_ZERO_2W.spi_bus;
pub const OLED_CS: u8 = RPI_ZERO_2W.oled_cs;
pub const OLED_DC: u8 = RPI_ZERO_2W.oled_dc;
pub const OLED_RST: u8 = RPI_ZERO_2W.oled_rst;
pub const OLED_SD_CS: u8 = RPI_ZERO_2W.oled_sd_cs;
pub const OLED_SD_CD: u8 = RPI_ZERO_2W.oled_sd_cd;

pub const I2S_BCK: u8 = RPI_ZERO_2W.i2s_bck;
pub const I2S_LRCK: u8 = RPI_ZERO_2W.i2s_lrck;
pub const I2S_DIN: u8 = RPI_ZERO_2W.i2s_din;

pub const ENCODERS: [EncoderPins; 1 + platform_core::AUX_ENCODER_COUNT] = RPI_ZERO_2W.encoders;

pub const NEOKEY_ADDR: u16 = RPI_ZERO_2W.neokey_addr;

pub const SEESAW_INT: u8 = RPI_ZERO_2W.seesaw_int;

pub const TRELLIS_ADDRS: [u16; 4] = RPI_ZERO_2W.trellis_addrs;
