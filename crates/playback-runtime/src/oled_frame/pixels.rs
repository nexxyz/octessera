use super::OLED_HEIGHT;

pub(super) fn fill_rect(
    frame: &mut [u8],
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    color: u16,
) {
    for py in y..(y + height).min(OLED_HEIGHT) {
        for px in x..(x + width).min(OLED_HEIGHT) {
            let index = (py * OLED_HEIGHT + px) * 2;
            frame[index] = (color >> 8) as u8;
            frame[index + 1] = color as u8;
        }
    }
}

pub(super) fn rgb565(rgb: [u8; 3]) -> u16 {
    ((u16::from(rgb[0]) & 0xF8) << 8) | ((u16::from(rgb[1]) & 0xFC) << 3) | (u16::from(rgb[2]) >> 3)
}

pub(super) fn rgb565_to_rgb(value: u16) -> [u8; 3] {
    [
        ((((value >> 11) & 0x1F) * 255) / 31) as u8,
        ((((value >> 5) & 0x3F) * 255) / 63) as u8,
        (((value & 0x1F) * 255) / 31) as u8,
    ]
}

pub(super) fn scale(rgb: [u8; 3], factor: f32) -> [u8; 3] {
    [
        ((rgb[0] as f32) * factor).round().clamp(0.0, 255.0) as u8,
        ((rgb[1] as f32) * factor).round().clamp(0.0, 255.0) as u8,
        ((rgb[2] as f32) * factor).round().clamp(0.0, 255.0) as u8,
    ]
}

pub(super) fn dim(rgb: [u8; 3], divisor: u8) -> [u8; 3] {
    let divisor = divisor.max(1);
    [rgb[0] / divisor, rgb[1] / divisor, rgb[2] / divisor]
}
