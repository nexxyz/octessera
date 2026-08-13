//! Hardware Abstraction Layer for octessera
//! Used by headless Pi Zero 2W binary (and optionally desktop for testing)

pub mod board_profiles;
pub mod encoder_gpio;
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub mod i2c_bus;
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub mod i2s_dac;
pub mod neokey;
pub mod neotrellis;
pub mod oled_ssd1351;
pub mod oled_startup_plan;
#[cfg(feature = "orange-pi-zero-2w")]
pub mod orange_encoder_gpio;
#[cfg(feature = "orange-pi-zero-2w")]
pub mod orange_hardware;
#[cfg(feature = "orange-pi-zero-2w")]
pub mod orange_metadata;
#[cfg(feature = "orange-pi-zero-2w")]
pub mod orange_timing;
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub mod pinmap;
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub mod seesaw_interrupt;
#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
pub(crate) mod seesaw_transport;

// Re-exports for convenience
#[cfg(any(
    feature = "raspberry-pi-zero-2w",
    all(
        not(feature = "raspberry-pi-zero-2w"),
        not(feature = "orange-pi-zero-2w")
    )
))]
pub use encoder_gpio::EncoderGpio;
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub use i2c_bus::I2CBus;
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub use i2s_dac::I2sDac;
pub use neokey::NeoKey;
pub use neotrellis::NeoTrellis;
pub use oled_ssd1351::OledSsd1351;
#[cfg(feature = "orange-pi-zero-2w")]
pub use orange_encoder_gpio::OrangeEncoderGpio;
#[cfg(not(feature = "orange-pi-zero-2w"))]
pub use seesaw_interrupt::SeesawInterrupt;

#[cfg(all(feature = "orange-pi-zero-2w", feature = "raspberry-pi-zero-2w"))]
compile_error!("select exactly one Octessera board profile");
