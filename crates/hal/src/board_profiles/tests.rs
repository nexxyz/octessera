use super::{ORANGE_PI_ZERO_2W_ID, RASPBERRY_PI_ZERO_2W, RASPBERRY_PI_ZERO_2W_ID, RPI_ZERO_2W};
use crate::pinmap;

#[test]
fn rpi_profile_matches_legacy_pinmap_constants() {
    assert_eq!(pinmap::I2C_BUS, RPI_ZERO_2W.i2c_bus);
    assert_eq!(pinmap::I2C_PATH, RPI_ZERO_2W.i2c_path);
    assert_eq!(pinmap::SPI_BUS, RPI_ZERO_2W.spi_bus);
    assert_eq!(pinmap::OLED_CS, RPI_ZERO_2W.oled_cs);
    assert_eq!(pinmap::OLED_DC, RPI_ZERO_2W.oled_dc);
    assert_eq!(pinmap::OLED_RST, RPI_ZERO_2W.oled_rst);
    assert_eq!(pinmap::OLED_SD_CS, RPI_ZERO_2W.oled_sd_cs);
    assert_eq!(pinmap::OLED_SD_CD, RPI_ZERO_2W.oled_sd_cd);
    assert_eq!(pinmap::I2S_BCK, RPI_ZERO_2W.i2s_bck);
    assert_eq!(pinmap::I2S_LRCK, RPI_ZERO_2W.i2s_lrck);
    assert_eq!(pinmap::I2S_DIN, RPI_ZERO_2W.i2s_din);
    assert_eq!(pinmap::ENCODERS, RPI_ZERO_2W.encoders);
    assert_eq!(pinmap::NEOKEY_ADDR, RPI_ZERO_2W.neokey_addr);
    assert_eq!(pinmap::SEESAW_INT, RPI_ZERO_2W.seesaw_int);
    assert_eq!(pinmap::TRELLIS_ADDRS, RPI_ZERO_2W.trellis_addrs);
}

#[test]
fn profile_ids_are_distinct_and_rpi_alias_is_stable() {
    assert_eq!(RASPBERRY_PI_ZERO_2W.id, RASPBERRY_PI_ZERO_2W_ID);
    assert_ne!(RASPBERRY_PI_ZERO_2W_ID, ORANGE_PI_ZERO_2W_ID);
    assert_eq!(RPI_ZERO_2W.id, RASPBERRY_PI_ZERO_2W_ID);
}
