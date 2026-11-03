use super::render::render_oled_frame;
use super::tests::{base, pixel};
use super::OledFont;

const HEADER: std::ops::Range<usize> = 0..9;
const BODY: std::ops::Range<usize> = 10..114;
const FOOTER: std::ops::Range<usize> = 114..128;

fn render(main: OledFont, small: OledFont) -> Vec<u8> {
    let mut input = base();
    input.display.main_font = main;
    input.display.small_font = small;
    render_oled_frame(&input)
}

fn differs(a: &[u8], b: &[u8], rows: std::ops::Range<usize>) -> bool {
    rows.into_iter()
        .any(|y| (0..128).any(|x| pixel(a, x, y) != pixel(b, x, y)))
}

#[test]
fn the_main_font_setting_switches_the_body_glyphs_and_nothing_else() {
    let edited = render(OledFont::Octessera, OledFont::Octessera);
    let original = render(OledFont::Original, OledFont::Octessera);
    assert!(differs(&edited, &original, BODY));
    assert!(!differs(&edited, &original, HEADER));
    assert!(!differs(&edited, &original, FOOTER));
}

#[test]
fn the_small_font_setting_switches_the_breadcrumb_and_hint_glyphs_and_nothing_else() {
    let edited = render(OledFont::Octessera, OledFont::Octessera);
    let original = render(OledFont::Octessera, OledFont::Original);
    assert!(differs(&edited, &original, HEADER));
    assert!(differs(&edited, &original, FOOTER));
    assert!(!differs(&edited, &original, BODY));
}

#[test]
fn the_default_font_setting_is_the_edited_glyph_set() {
    assert_eq!(OledFont::default(), OledFont::Octessera);
    assert_eq!(OledFont::from_setting("original"), OledFont::Original);
    assert_eq!(OledFont::from_setting("octessera"), OledFont::Octessera);
}
