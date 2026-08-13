use super::oled_error_tests::menu_snapshot;
use serde_json::{json, Value};

pub(super) const GLYPH_DIGITS_FNV1A64: u64 = 0x9FE1_8ECD_428C_BC3F;
pub(super) const GLYPH_UPPERCASE_FIRST_FNV1A64: u64 = 0x81AD_DF18_3C6F_3865;
pub(super) const GLYPH_UPPERCASE_LAST_FNV1A64: u64 = 0x4D26_9228_40F7_11A5;
pub(super) const GLYPH_LOWERCASE_FIRST_FNV1A64: u64 = 0xD1AA_9D54_9F8E_8067;
pub(super) const GLYPH_LOWERCASE_LAST_FNV1A64: u64 = 0x1472_784E_157A_10A5;
pub(super) const GLYPH_PUNCTUATION_FIRST_FNV1A64: u64 = 0x67F4_248A_C2B9_8D85;
pub(super) const GLYPH_PUNCTUATION_LAST_FNV1A64: u64 = 0x1662_9A94_8669_A771;
pub(super) const GLYPH_SYMBOLS_FNV1A64: u64 = 0x9416_2B3D_DEEE_A9EF;

pub(super) const GLYPH_FIXTURES: &[GlyphFixture] = &[
    GlyphFixture {
        name: "glyph-digits",
        text: "0123456789",
    },
    GlyphFixture {
        name: "glyph-uppercase-first",
        text: "ABCDEFGHIJKLMNOPQRS",
    },
    GlyphFixture {
        name: "glyph-uppercase-last",
        text: "TUVWXYZ",
    },
    GlyphFixture {
        name: "glyph-lowercase-first",
        text: "abcdefghijklmnopqrs",
    },
    GlyphFixture {
        name: "glyph-lowercase-last",
        text: "tuvwxyz",
    },
    GlyphFixture {
        name: "glyph-punctuation-first",
        text: ":.-*+/()_#@><[]%!",
    },
    GlyphFixture {
        name: "glyph-punctuation-last",
        text: "?,\'\"=",
    },
    GlyphFixture {
        name: "glyph-symbols",
        text: "▶■●|",
    },
];

pub(super) struct GlyphFixture {
    pub(super) name: &'static str,
    pub(super) text: &'static str,
}

pub(super) fn assert_glyph_fixture_coverage() {
    for ch in DEFINED_GLYPHS.chars() {
        let (fixture, position) = glyph_fixture_position(ch)
            .unwrap_or_else(|| panic!("glyph {ch:?} is not covered by an unclipped fixture"));
        assert!(
            position < 19,
            "glyph {ch:?} exceeds fixture clipping: {fixture}"
        );
        assert_eq!(
            GLYPH_FIXTURES
                .iter()
                .find(|candidate| candidate.name == fixture)
                .and_then(|candidate| candidate.text.chars().nth(position)),
            Some(ch),
            "glyph {ch:?} fixture mapping is stale"
        );
    }
    assert!(GLYPH_FIXTURES
        .iter()
        .all(|fixture| fixture.text.chars().count() <= 19));
}

const DEFINED_GLYPHS: &str =
    "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz:.-*+/()_#@><[]%!?,\'\"=▶■●|";

fn glyph_fixture_position(ch: char) -> Option<(&'static str, usize)> {
    GLYPH_FIXTURES.iter().find_map(|fixture| {
        fixture
            .text
            .chars()
            .position(|fixture_ch| fixture_ch == ch)
            .map(|position| (fixture.name, position))
    })
}

pub(super) fn glyph_snapshot(fixture: &GlyphFixture) -> Value {
    let mut snapshot = menu_snapshot();
    snapshot["display"]["title"] = json!("Glyphs");
    snapshot["display"]["lines"] = json!([
        fixture.text,
        fixture.text,
        fixture.text,
        fixture.text,
        fixture.text,
        fixture.text,
        fixture.text
    ]);
    snapshot["display"]["colors"] = json!([
        platform_core::palette::WHITE_RGB565,
        platform_core::palette::WHITE_RGB565,
        platform_core::palette::WHITE_RGB565,
        platform_core::palette::WHITE_RGB565,
        platform_core::palette::WHITE_RGB565,
        platform_core::palette::WHITE_RGB565,
        platform_core::palette::WHITE_RGB565
    ]);
    snapshot["display"]["barValues"] = json!([null, null, null, null, null, null, null]);
    snapshot["selectedRow"] = Value::Null;
    snapshot
}
