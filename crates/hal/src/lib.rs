//! Hardware Abstraction Layer for octessera
//! Used by headless Pi Zero 2W binary (and optionally desktop for testing)

pub mod board_profiles;
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub mod encoder_gpio;
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub mod i2c_bus;
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub mod i2s_dac;
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub mod neokey;
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub mod neotrellis;
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub mod oled_ssd1351;
#[cfg(feature = "orange-pi-zero-2w")]
pub mod orange_hardware;
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub mod pinmap;
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub mod seesaw_interrupt;

// Re-exports for convenience
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub use encoder_gpio::EncoderGpio;
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub use i2c_bus::I2CBus;
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub use i2s_dac::I2sDac;
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub use neokey::NeoKey;
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub use neotrellis::NeoTrellis;
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub use oled_ssd1351::OledSsd1351;
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub use seesaw_interrupt::SeesawInterrupt;

#[cfg(any(
    all(feature = "orange-pi-zero-2w", feature = "raspberry-pi-zero-2w"),
    all(feature = "orange-pi-zero-2w", feature = "rpi-zero-2w"),
    all(feature = "orange-pi-zero-2w", feature = "pi-zero"),
))]
compile_error!("select exactly one Octessera board profile");
