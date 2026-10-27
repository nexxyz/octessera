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
    pub reverse_direction: bool,
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
            reverse_direction: true,
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
            reverse_direction: true,
        }
    }

    pub const fn switch_available(self, uart0_active: bool) -> bool {
        !uart0_active || self.uart_conflict.is_none()
    }

    pub const fn requested_gpio_offsets(self, uart0_active: bool) -> [Option<u32>; 3] {
        [
            Some(self.a),
            Some(self.b),
            if self.switch_available(uart0_active) {
                Some(self.sw)
            } else {
                None
            },
        ]
    }

    pub const fn canonical_turn_delta(self, delta: i8) -> i8 {
        if self.reverse_direction {
            -delta
        } else {
            delta
        }
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
    pub seesaw_int: u32,
    pub encoders: [OrangeEncoderPins; 1 + platform_core::AUX_ENCODER_COUNT],
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
        seesaw_int: 225,
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
mod tests;

#[cfg(all(test, feature = "orange-pi-zero-2w"))]
mod orange_tests;
