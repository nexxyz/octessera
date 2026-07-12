#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LedColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl LedColor {
    pub const BLACK: Self = Self::from_rgb(platform_core::palette::BLACK);
    pub const RED: Self = Self::from_rgb(platform_core::palette::RED);
    pub const YELLOW: Self = Self::from_rgb(platform_core::palette::YELLOW);
    pub const SYSTEM: Self = Self::from_rgb(platform_core::palette::GRAY);
    pub const BLUE: Self = Self::from_rgb(platform_core::palette::BLUE);
    pub const WHITE: Self = Self::from_rgb(platform_core::palette::WHITE);
    pub const GREEN: Self = Self::from_rgb(platform_core::palette::GREEN);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const fn from_rgb(rgb: [u8; 3]) -> Self {
        Self::rgb(rgb[0], rgb[1], rgb[2])
    }

    pub fn dim(self, divisor: u8) -> Self {
        let divisor = divisor.max(1);
        Self::rgb(self.r / divisor, self.g / divisor, self.b / divisor)
    }

    pub fn add_dim_white(self, amount: u8) -> Self {
        Self::rgb(
            self.r.saturating_add(amount),
            self.g.saturating_add(amount),
            self.b.saturating_add(amount),
        )
    }

    pub fn append_rgb(self, target: &mut Vec<u8>) {
        target.extend([self.r, self.g, self.b]);
    }
}

pub(super) fn trigger_gate_color(mode: &str) -> LedColor {
    match mode {
        "zero" => LedColor::RED,
        "custom" => LedColor::YELLOW,
        _ => LedColor::GREEN,
    }
}
