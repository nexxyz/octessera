#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EncoderPins {
    pub a: u8,
    pub b: u8,
    pub sw: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrangeUartConflict {
    pub signal: &'static str,
    pub physical_pin: u8,
    pub offset: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrangeEncoderPins {
    pub physical_a: u8,
    pub physical_b: u8,
    pub physical_sw: u8,
    pub a: u32,
    pub b: u32,
    pub sw: u32,
    pub uart_conflict: Option<OrangeUartConflict>,
}

impl OrangeEncoderPins {
    pub const fn qualified(
        physical_a: u8,
        physical_b: u8,
        physical_sw: u8,
        a: u32,
        b: u32,
        sw: u32,
    ) -> Self {
        Self {
            physical_a,
            physical_b,
            physical_sw,
            a,
            b,
            sw,
            uart_conflict: None,
        }
    }

    pub const fn with_uart_conflict(
        physical_a: u8,
        physical_b: u8,
        physical_sw: u8,
        a: u32,
        b: u32,
        sw: u32,
        signal: &'static str,
    ) -> Self {
        Self {
            physical_a,
            physical_b,
            physical_sw,
            a,
            b,
            sw,
            uart_conflict: Some(OrangeUartConflict {
                signal,
                physical_pin: physical_sw,
                offset: sw,
            }),
        }
    }

    pub const fn is_qualified(self) -> bool {
        self.uart_conflict.is_none()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceDescriptor {
    pub path: &'static str,
    pub controller: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrangeGpioDescriptor {
    pub chip_label: &'static str,
    pub dc_offset: u32,
    pub reset_offset: u32,
    pub reset_active_low: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrangePiDevices {
    pub i2c: DeviceDescriptor,
    pub trellis_addrs: [u16; 4],
    pub neokey_addr: u16,
    pub spi: DeviceDescriptor,
    pub gpio: OrangeGpioDescriptor,
    pub encoders: [OrangeEncoderPins; 1 + platform_core::AUX_ENCODER_COUNT],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeesawInputMode {
    Interrupt,
    Polling,
}

pub const RASPBERRY_PI_ZERO_2W_ID: &str = "raspberry-pi-zero-2w";
pub const ORANGE_PI_ZERO_2W_ID: &str = "orange-pi-zero-2w";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoardProfile {
    pub id: &'static str,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrangeBoardProfile {
    pub id: &'static str,
    pub devices: OrangePiDevices,
}

pub const RASPBERRY_PI_ZERO_2W: BoardProfile = BoardProfile {
    id: RASPBERRY_PI_ZERO_2W_ID,
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

pub const ORANGE_PI_ZERO_2W: OrangeBoardProfile = OrangeBoardProfile {
    id: ORANGE_PI_ZERO_2W_ID,
    devices: OrangePiDevices {
        i2c: DeviceDescriptor {
            path: "/dev/i2c-2",
            controller: "5002400.i2c",
        },
        trellis_addrs: [0x2E, 0x2F, 0x30, 0x31],
        neokey_addr: 0x3F,
        spi: DeviceDescriptor {
            path: "/dev/spidev1.0",
            controller: "5011000.spi",
        },
        gpio: OrangeGpioDescriptor {
            chip_label: "300b000.pinctrl",
            dc_offset: 270,
            reset_offset: 76,
            reset_active_low: true,
        },
        encoders: [
            OrangeEncoderPins::qualified(29, 31, 32, 256, 271, 267),
            OrangeEncoderPins::qualified(33, 22, 11, 268, 262, 226),
            OrangeEncoderPins::with_uart_conflict(13, 7, 8, 227, 269, 224, "UART0 TX"),
            OrangeEncoderPins::qualified(37, 18, 15, 272, 228, 261),
        ],
    },
};

pub const ORANGE_PI_ZERO_2W_DEVICES: OrangePiDevices = ORANGE_PI_ZERO_2W.devices;

pub const RPI_ZERO_2W: BoardProfile = RASPBERRY_PI_ZERO_2W;

#[cfg(feature = "orange-pi-zero-2w")]
pub const ACTIVE_BOARD_PROFILE: OrangeBoardProfile = ORANGE_PI_ZERO_2W;

#[cfg(not(feature = "orange-pi-zero-2w"))]
pub const ACTIVE_BOARD_PROFILE: BoardProfile = RASPBERRY_PI_ZERO_2W;

pub const ACTIVE_BOARD_PROFILE_ID: &str = ACTIVE_BOARD_PROFILE.id;

#[cfg(all(test, not(feature = "orange-pi-zero-2w")))]
mod tests {
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
}

#[cfg(all(test, feature = "orange-pi-zero-2w"))]
mod orange_tests {
    use super::{
        OrangeUartConflict, ACTIVE_BOARD_PROFILE, ORANGE_PI_ZERO_2W, ORANGE_PI_ZERO_2W_DEVICES,
        ORANGE_PI_ZERO_2W_ID,
    };

    #[test]
    fn orange_profile_selects_typed_linux_devices_without_raspberry_pins() {
        assert_eq!(ACTIVE_BOARD_PROFILE, ORANGE_PI_ZERO_2W);
        assert_eq!(ORANGE_PI_ZERO_2W.id, ORANGE_PI_ZERO_2W_ID);
        assert_eq!(ORANGE_PI_ZERO_2W_DEVICES, ORANGE_PI_ZERO_2W.devices);
        assert_eq!(ORANGE_PI_ZERO_2W.devices.i2c.path, "/dev/i2c-2");
        assert_eq!(ORANGE_PI_ZERO_2W.devices.i2c.controller, "5002400.i2c");
        assert_eq!(
            ORANGE_PI_ZERO_2W.devices.trellis_addrs,
            [0x2E, 0x2F, 0x30, 0x31]
        );
        assert_eq!(ORANGE_PI_ZERO_2W.devices.neokey_addr, 0x3F);
        assert_eq!(ORANGE_PI_ZERO_2W.devices.spi.path, "/dev/spidev1.0");
        assert_eq!(ORANGE_PI_ZERO_2W.devices.spi.controller, "5011000.spi");
        assert_eq!(ORANGE_PI_ZERO_2W.devices.gpio.chip_label, "300b000.pinctrl");
        assert_eq!(ORANGE_PI_ZERO_2W.devices.gpio.dc_offset, 270);
        assert_eq!(ORANGE_PI_ZERO_2W.devices.gpio.reset_offset, 76);
    }

    #[test]
    fn orange_encoder_header_mapping_matches_h618_offsets() {
        let encoders = ORANGE_PI_ZERO_2W_DEVICES.encoders;
        assert_eq!(
            encoders.map(|pins| (
                (pins.physical_a, pins.physical_b, pins.physical_sw),
                (pins.a, pins.b, pins.sw),
            )),
            [
                ((29, 31, 32), (256, 271, 267)),
                ((33, 22, 11), (268, 262, 226)),
                ((13, 7, 8), (227, 269, 224)),
                ((37, 18, 15), (272, 228, 261)),
            ]
        );
    }

    #[test]
    fn orange_uart_encoder_line_is_excluded_until_boot_routing_changes() {
        let encoders = ORANGE_PI_ZERO_2W_DEVICES.encoders;
        assert!(encoders[0].is_qualified());
        assert!(encoders[1].is_qualified());
        assert_eq!(
            encoders[2].uart_conflict,
            Some(OrangeUartConflict {
                signal: "UART0 TX",
                physical_pin: 8,
                offset: 224,
            })
        );
        assert!(!encoders[2].is_qualified());
        assert!(encoders[3].is_qualified());
    }
}
