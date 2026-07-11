include!(concat!(env!("OUT_DIR"), "/display_palette.generated.rs"));

pub const DEFAULT_BEHAVIOR_ACTIVE: [u8; 3] = SPARKS;
pub const DEFAULT_BEHAVIOR_INACTIVE: [u8; 3] = BLACK;
pub const DEFAULT_BEHAVIOR_STABLE: [u8; 3] = WORLDS;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_palette_matches_canonical_values() {
        assert_eq!(WORLDS, [99, 210, 63]);
        assert_eq!(PULSES, [224, 119, 204]);
        assert_eq!(TONES, [53, 207, 242]);
        assert_eq!(SPARKS, [255, 212, 71]);
        assert_eq!(SYSTEM, [201, 206, 214]);
        assert_eq!(WHITE, [255, 255, 255]);
        assert_eq!(BLACK, [0, 0, 0]);
        assert_eq!(WORLDS_RGB565, 0x6687);
        assert_eq!(PULSES_RGB565, 0xE3B9);
        assert_eq!(TONES_RGB565, 0x367E);
        assert_eq!(SPARKS_RGB565, 0xFEA8);
        assert_eq!(SYSTEM_RGB565, 0xCE7A);
        assert_eq!(WHITE_RGB565, 0xFFFF);
        assert_eq!(BLACK_RGB565, 0x0000);
    }
}
