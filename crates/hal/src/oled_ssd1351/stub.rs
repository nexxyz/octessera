#[cfg(not(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w")))]
use std::fmt;
use std::ops::Range;

/// The Orange driver on a non-Linux build: every call fails.
#[cfg(all(feature = "orange-pi-zero-2w", not(target_os = "linux")))]
pub struct OledSsd1351;

#[cfg(all(feature = "orange-pi-zero-2w", not(target_os = "linux")))]
impl OledSsd1351 {
    pub fn new() -> Result<Self, String> {
        Err("Orange OLED requires a Linux target".into())
    }

    pub fn adopt_existing() -> Result<Self, String> {
        Err("Orange OLED requires a Linux target".into())
    }

    pub fn detach_preserving(&mut self) -> Result<(), String> {
        Err("Orange OLED requires a Linux target".into())
    }

    pub fn reacquire_existing(&mut self) -> Result<(), String> {
        Err("Orange OLED requires a Linux target".into())
    }

    pub fn write_frame(&mut self, _pixels: &[u8]) -> Result<(), String> {
        Err("Orange OLED requires a Linux target".into())
    }

    pub fn write_rows(&mut self, _pixels: &[u8], _rows: Range<usize>) -> Result<(), String> {
        Err("Orange OLED requires a Linux target".into())
    }

    pub fn display_on(&mut self) -> Result<(), String> {
        Err("Orange OLED requires a Linux target".into())
    }

    pub fn display_off(&mut self) -> Result<(), String> {
        Err("Orange OLED requires a Linux target".into())
    }

    pub fn set_master_contrast(&mut self, _step: u8) -> Result<(), String> {
        Err("Orange OLED requires a Linux target".into())
    }
}

/// A desktop or test build without a board: every call succeeds and draws nothing.
#[cfg(not(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w")))]
pub struct OledSsd1351 {
    _private: (),
}

#[cfg(not(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w")))]
impl OledSsd1351 {
    pub fn new() -> Result<Self, String> {
        Ok(Self { _private: () })
    }

    pub fn adopt_existing() -> Result<Self, String> {
        Self::new()
    }

    pub fn detach_preserving(&mut self) -> Result<(), String> {
        Ok(())
    }

    pub fn reacquire_existing(&mut self) -> Result<(), String> {
        Ok(())
    }

    pub fn write_frame(&mut self, _pixels: &[u8]) -> Result<(), String> {
        Ok(())
    }

    pub fn write_rows(&mut self, _pixels: &[u8], _rows: Range<usize>) -> Result<(), String> {
        Ok(())
    }

    pub fn display_on(&mut self) -> Result<(), String> {
        Ok(())
    }

    pub fn display_off(&mut self) -> Result<(), String> {
        Ok(())
    }

    pub fn set_master_contrast(&mut self, _step: u8) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(not(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w")))]
impl fmt::Debug for OledSsd1351 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OledSsd1351 {{ ... }}")
    }
}
