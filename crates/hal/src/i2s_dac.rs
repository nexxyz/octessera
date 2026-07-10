//! I2S PCM5102 DAC driver for Raspberry Pi Zero 2W
//! Hardware abstraction - actual audio output is in the Pi app via rodio

#[cfg(not(feature = "rpi-zero-2w"))]
use std::fmt;

/// I2S DAC wrapper - stub for non-Pi builds
/// Actual audio output uses rodio in the Pi-Zero app
#[cfg(feature = "rpi-zero-2w")]
pub struct I2sDac {
    // Placeholder - actual implementation in apps/pi-zero with rodio
}

#[cfg(feature = "rpi-zero-2w")]
impl I2sDac {
    /// Initialize I2S DAC
    pub fn new() -> Result<Self, String> {
        // PCM5102 is a dumb DAC that auto-detects I2S clock
        // Actual rodio initialization happens in the Pi app
        Ok(Self {})
    }

    /// Trigger a note (stub - actual implementation in Pi app)
    pub fn trigger_note(
        &self,
        _channel: u8,
        _note: u8,
        _velocity: u8,
        _duration_ms: u32,
    ) -> Result<(), String> {
        // Note: actual audio rendering is done by realtime-engine crate
        // This is just a placeholder for the HAL interface
        Ok(())
    }
}

/// Stub for non-Pi builds
#[cfg(not(feature = "rpi-zero-2w"))]
pub struct I2sDac {
    _private: (),
}

#[cfg(not(feature = "rpi-zero-2w"))]
impl I2sDac {
    pub fn new() -> Result<Self, String> {
        Ok(Self { _private: () })
    }

    pub fn trigger_note(
        &self,
        _channel: u8,
        _note: u8,
        _velocity: u8,
        _duration_ms: u32,
    ) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(not(feature = "rpi-zero-2w"))]
impl fmt::Debug for I2sDac {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "I2sDac {{ ... }}")
    }
}
