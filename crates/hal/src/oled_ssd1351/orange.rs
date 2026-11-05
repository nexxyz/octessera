use super::{rotate_clockwise_rgb565, rotated_row_window, FRAME_BYTES};
use std::ops::Range;

pub struct OledSsd1351 {
    transport: Option<crate::orange_hardware::OrangeOledTransport>,
    rotated_frame: Vec<u8>,
    window: Vec<u8>,
}

impl OledSsd1351 {
    pub fn new() -> Result<Self, String> {
        Self::open(false)
    }

    pub fn adopt_existing() -> Result<Self, String> {
        Self::open(true)
    }

    fn open(preserve_existing: bool) -> Result<Self, String> {
        let startup_plan = crate::oled_startup_plan::OledStartupPlan::new(preserve_existing);
        let hardware = if startup_plan.operations().is_empty() {
            crate::orange_hardware::OrangeHardware::open_preserve_existing()?
        } else {
            crate::orange_hardware::OrangeHardware::open()?
        };
        Ok(Self {
            transport: Some(hardware.into_oled()),
            rotated_frame: vec![0_u8; FRAME_BYTES],
            window: Vec::with_capacity(FRAME_BYTES),
        })
    }

    pub fn write_frame(&mut self, pixels: &[u8]) -> Result<(), String> {
        let frame = rotate_clockwise_rgb565(pixels, &mut self.rotated_frame);
        self.transport
            .as_mut()
            .ok_or_else(|| "Orange OLED hardware is detached".to_string())?
            .write_frame(frame)
    }

    /// Writes only logical rows `rows` of a pre-rendered frame, as one RAM window.
    pub fn write_rows(&mut self, pixels: &[u8], rows: Range<usize>) -> Result<(), String> {
        let columns = rotated_row_window(pixels, rows, &mut self.window);
        self.transport
            .as_mut()
            .ok_or_else(|| "Orange OLED hardware is detached".to_string())?
            .write_columns(columns, &self.window)
    }

    pub fn display_on(&mut self) -> Result<(), String> {
        self.transport
            .as_mut()
            .ok_or_else(|| "Orange OLED hardware is detached".to_string())?
            .display_on()
    }

    pub fn display_off(&mut self) -> Result<(), String> {
        self.transport
            .as_mut()
            .ok_or_else(|| "Orange OLED hardware is detached".to_string())?
            .display_off()
    }

    pub fn set_master_contrast(&mut self, step: u8) -> Result<(), String> {
        self.transport
            .as_mut()
            .ok_or_else(|| "Orange OLED hardware is detached".to_string())?
            .set_master_contrast(step)
    }

    pub fn detach_preserving(&mut self) -> Result<(), String> {
        let transport = self
            .transport
            .take()
            .ok_or_else(|| "Orange OLED hardware is already detached".to_string())?;
        transport.detach_preserving();
        Ok(())
    }

    pub fn reacquire_existing(&mut self) -> Result<(), String> {
        if self.transport.is_some() {
            return Err("Orange OLED hardware is already attached".into());
        }
        let hardware = crate::orange_hardware::OrangeHardware::open_preserve_existing()?;
        self.transport = Some(hardware.into_oled());
        Ok(())
    }
}
