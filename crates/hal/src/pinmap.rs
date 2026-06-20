//! Pin mapping for Cell Symphony hardware on Pi Zero 2W

/// I2C Bus 1 (GPIO2=SDA, GPIO3=SCL)
pub const I2C_BUS: u8 = 1;

/// SPI Bus 0
pub const SPI_BUS: &str = "/dev/spidev0.0";
pub const OLED_CS: u8 = 8; // GPIO8/CE0
pub const OLED_DC: u8 = 24; // GPIO24
pub const OLED_RST: u8 = 25; // GPIO25

/// I2S Pins
pub const I2S_BCK: u8 = 18; // GPIO18
pub const I2S_LRCK: u8 = 19; // GPIO19
pub const I2S_DIN: u8 = 21; // GPIO21

/// Encoder pins (A, B, Switch)
#[derive(Clone, Copy)]
pub struct EncoderPins {
    pub a: u8,
    pub b: u8,
    pub sw: u8,
}

/// Rotary encoders on Pi Zero 2W: main plus aux controls.
pub const ENCODERS: [EncoderPins; 1 + platform_core::AUX_ENCODER_COUNT] = [
    EncoderPins { a: 5, b: 6, sw: 12 }, // SW1 (main)
    EncoderPins {
        a: 13,
        b: 16,
        sw: 17,
    }, // SW2 (aux1)
    EncoderPins {
        a: 27,
        b: 4,
        sw: 20,
    }, // SW3 (aux2)
    EncoderPins {
        a: 26,
        b: 23,
        sw: 22,
    }, // SW4 (aux3)
];

/// NeoKey I2C address (typically 0x30)
pub const NEOKEY_ADDR: u16 = 0x30;

/// NeoTrellis I2C addresses (4 devices, 4x4 each = 8x8 grid)
pub const TRELLIS_ADDRS: [u16; 4] = [0x2E, 0x2F, 0x30, 0x31];
