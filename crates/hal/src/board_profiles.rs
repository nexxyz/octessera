#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EncoderPins {
    pub a: u8,
    pub b: u8,
    pub sw: u8,
}

pub struct BoardProfile {
    pub i2c_bus: u8,
    pub i2c_path: &'static str,
    pub spi_bus: &'static str,
    pub oled_cs: u8,
    pub oled_dc: u8,
    pub oled_rst: u8,
    pub oled_sd_cs: u8,
    pub oled_sd_cd: u8,
    pub i2s_bck: u8,
    pub i2s_lrck: u8,
    pub i2s_din: u8,
    pub encoders: [EncoderPins; 1 + platform_core::AUX_ENCODER_COUNT],
    pub neokey_addr: u16,
    pub seesaw_int: u8,
    pub trellis_addrs: [u16; 4],
}

pub const RPI_ZERO_2W: BoardProfile = BoardProfile {
    i2c_bus: 1,
    i2c_path: "/dev/i2c-1",
    spi_bus: "/dev/spidev0.0",
    oled_cs: 8,
    oled_dc: 23,
    oled_rst: 16,
    oled_sd_cs: 7,
    oled_sd_cd: 20,
    i2s_bck: 18,
    i2s_lrck: 19,
    i2s_din: 21,
    encoders: [
        EncoderPins { a: 6, b: 5, sw: 12 },
        EncoderPins {
            a: 25,
            b: 13,
            sw: 17,
        },
        EncoderPins {
            a: 4,
            b: 27,
            sw: 14,
        },
        EncoderPins {
            a: 24,
            b: 26,
            sw: 22,
        },
    ],
    neokey_addr: 0x3F,
    seesaw_int: 15,
    trellis_addrs: [0x2E, 0x2F, 0x30, 0x31],
};

#[cfg(test)]
mod tests {
    use super::RPI_ZERO_2W;
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
}
