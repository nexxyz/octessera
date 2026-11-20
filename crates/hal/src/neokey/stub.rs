pub struct NeoKey {
    _private: (),
}

impl NeoKey {
    pub fn new(_i2c_path: &str) -> Result<Self, String> {
        Ok(Self { _private: () })
    }

    pub fn scan(&mut self) -> Result<Vec<(u8, bool)>, String> {
        Ok(Vec::new())
    }

    pub fn settling(&self) -> bool {
        false
    }

    pub fn scan_interrupts(&mut self) -> Result<Vec<(u8, bool)>, String> {
        self.scan()
    }

    pub fn take_led_refresh(&mut self) -> bool {
        false
    }

    pub fn reapply_setup_part(&mut self, _part: usize) -> Result<bool, String> {
        Ok(false)
    }

    pub fn raw_button_state(&mut self) -> Result<u32, String> {
        Ok(0)
    }

    pub fn set_led(&mut self, _key: u8, _r: u8, _g: u8, _b: u8) -> Result<(), String> {
        Ok(())
    }
}

impl std::fmt::Debug for NeoKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NeoKey {{ ... }}")
    }
}
