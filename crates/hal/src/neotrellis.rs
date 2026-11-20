//! NeoTrellis 8x8 LED matrix driver (4x4 devices x4 chain).

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
mod hardware;
#[cfg(not(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test)))]
mod stub;

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
pub use hardware::NeoTrellis;
#[cfg(not(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test)))]
pub use stub::NeoTrellis;
