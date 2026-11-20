#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
mod hardware;
#[cfg(not(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test)))]
mod stub;

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test))]
pub use hardware::NeoKey;
#[cfg(not(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w", test)))]
pub use stub::NeoKey;
