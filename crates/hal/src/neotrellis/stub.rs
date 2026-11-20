pub struct NeoTrellis {
    _private: (),
}

impl NeoTrellis {
    pub fn new(_i2c_path: &str) -> Result<Self, String> {
        Ok(Self { _private: () })
    }

    pub fn scan_keys(&mut self, _events: &mut Vec<(usize, usize, bool)>) -> Result<(), String> {
        Ok(())
    }

    pub fn take_led_refresh(&mut self) -> bool {
        false
    }

    pub fn reapply_setup_part(&mut self, _dev_idx: usize, _part: usize) -> Result<bool, String> {
        Ok(false)
    }

    pub fn write_led_frame(&mut self, _frame: &[[u8; 3]; 64]) -> Result<(), String> {
        Ok(())
    }
}

impl std::fmt::Debug for NeoTrellis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NeoTrellis {{ ... }}")
    }
}
