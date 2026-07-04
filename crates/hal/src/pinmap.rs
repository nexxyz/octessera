//! Pin mapping for Cell Symphony hardware on Pi Zero 2W

/// I2C Bus 1 (GPIO2=SDA, GPIO3=SCL)
pub const I2C_BUS: u8 = 1;

/// SPI Bus 0
pub const SPI_BUS: &str = "/dev/spidev0.0";
pub const OLED_CS: u8 = 8; // GPIO8/CE0
pub const OLED_DC: u8 = 23; // GPIO23
pub const OLED_RST: u8 = 16; // GPIO16
pub const OLED_SD_CS: u8 = 7; // GPIO7/CE1
pub const OLED_SD_CD: u8 = 20; // GPIO20/card detect

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
    EncoderPins { a: 6, b: 5, sw: 12 }, // SW1 (main)
    EncoderPins {
        a: 25,
        b: 13,
        sw: 17,
    }, // SW2 (aux1)
    EncoderPins {
        a: 4,
        b: 27,
        sw: 14,
    }, // SW3 (aux2)
    EncoderPins {
        a: 24,
        b: 26,
        sw: 22,
    }, // SW4 (aux3)
];

/// NeoKey I2C address with A0, A1, A2, and A3 jumpers soldered.
pub const NEOKEY_ADDR: u16 = 0x3F;

/// Shared active-low interrupt line for NeoTrellis and NeoKey seesaw devices.
pub const SEESAW_INT: u8 = 15;

/// NeoTrellis I2C addresses ordered left-to-right, top-to-bottom.
pub const TRELLIS_ADDRS: [u16; 4] = [0x2E, 0x2F, 0x30, 0x31];
