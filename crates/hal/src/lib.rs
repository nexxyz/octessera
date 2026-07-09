//! Hardware Abstraction Layer for Octessera
//! Used by headless Pi Zero 2W binary (and optionally desktop for testing)

pub mod encoder_gpio;
pub mod i2c_bus;
pub mod i2s_dac;
pub mod neokey;
pub mod neotrellis;
pub mod oled_ssd1351;
pub mod pinmap;
pub mod seesaw_interrupt;

// Re-exports for convenience
pub use encoder_gpio::EncoderGpio;
pub use i2c_bus::I2CBus;
pub use i2s_dac::I2sDac;
pub use neokey::NeoKey;
pub use neotrellis::NeoTrellis;
pub use oled_ssd1351::OledSsd1351;
pub use seesaw_interrupt::SeesawInterrupt;
