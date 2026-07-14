include!(concat!(env!("OUT_DIR"), "/display_palette.generated.rs"));

pub const DEFAULT_BEHAVIOR_ACTIVE: [u8; 3] = YELLOW;
pub const DEFAULT_BEHAVIOR_INACTIVE: [u8; 3] = BLACK;
pub const DEFAULT_BEHAVIOR_STABLE: [u8; 3] = GREEN;

pub const BEHAVIOR_PRIMARY_BLUE: [u8; 3] = [0, 0, 255];
pub const BEHAVIOR_PRIMARY_GREEN: [u8; 3] = [0, 255, 0];
pub const BEHAVIOR_PRIMARY_YELLOW: [u8; 3] = [255, 255, 0];
pub const BEHAVIOR_PRIMARY_RED: [u8; 3] = [255, 0, 0];
pub const BEHAVIOR_DIM_GREEN: [u8; 3] = [0, 48, 0];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_palette_matches_canonical_values() {
        assert_eq!(GREEN, [99, 210, 63]);
        assert_eq!(RED, [221, 130, 205]);
        assert_eq!(BLUE, [53, 207, 242]);
        assert_eq!(YELLOW, [255, 212, 71]);
        assert_eq!(GRAY, [201, 206, 214]);
        assert_eq!(WHITE, [255, 255, 255]);
        assert_eq!(BLACK, [0, 0, 0]);
        assert_eq!(GREEN_RGB565, 0x6687);
        assert_eq!(RED_RGB565, 0xDC19);
        assert_eq!(BLUE_RGB565, 0x367E);
        assert_eq!(YELLOW_RGB565, 0xFEA8);
        assert_eq!(GRAY_RGB565, 0xCE7A);
        assert_eq!(WHITE_RGB565, 0xFFFF);
        assert_eq!(BLACK_RGB565, 0x0000);
        assert_eq!(BEHAVIOR_PRIMARY_GREEN, [0, 255, 0]);
        assert_eq!(BEHAVIOR_DIM_GREEN, [0, 48, 0]);
    }
}
