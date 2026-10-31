/// The SSD1351 master contrast current (command C7h) has 16 steps.
pub const OLED_MASTER_CONTRAST_STEPS: u8 = 16;

/// The master contrast step (0..=15) that drives the panel for an OLED Bright percentage:
/// the step whose share of full current, `(step + 1) / 16`, lies nearest the percentage.
pub fn oled_master_contrast_step(display_brightness: u8) -> u8 {
    let percent = u16::from(display_brightness.min(100));
    let steps = u16::from(OLED_MASTER_CONTRAST_STEPS);
    let nearest = (percent * steps + 50) / 100;
    nearest.saturating_sub(1).min(steps - 1) as u8
}

#[cfg(test)]
mod tests {
    use super::oled_master_contrast_step;

    #[test]
    fn oled_bright_percentages_map_to_master_contrast_steps() {
        let table = [
            (0, 0),
            (5, 0),
            (10, 1),
            (15, 1),
            (20, 2),
            (25, 3),
            (30, 4),
            (35, 5),
            (40, 5),
            (45, 6),
            (50, 7),
            (55, 8),
            (60, 9),
            (65, 9),
            (70, 10),
            (75, 11),
            (80, 12),
            (85, 13),
            (90, 13),
            (95, 14),
            (100, 15),
            (255, 15),
        ];
        for (percent, step) in table {
            assert_eq!(oled_master_contrast_step(percent), step, "{percent}%");
        }
    }
}
