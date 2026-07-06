#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LedColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl LedColor {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
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
        "zero" => LedColor::rgb(220, 0, 0),
        "custom" => LedColor::rgb(220, 180, 0),
        _ => LedColor::rgb(0, 220, 0),
    }
}
