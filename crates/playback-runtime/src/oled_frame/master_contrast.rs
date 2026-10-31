/// The SSD1351 master contrast current (command C7h) has 16 steps.
pub const OLED_MASTER_CONTRAST_STEPS: u8 = 16;

/// OLED Bright 1..=16 picks master contrast step 0..=15, one step per detent.
pub fn oled_master_contrast_step(display_brightness: u8) -> u8 {
    display_brightness.clamp(1, OLED_MASTER_CONTRAST_STEPS) - 1
}
