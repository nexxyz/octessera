use super::{
    fit_line_ellipsis, force_line_ellipsis, layout_card_body, normalize_text, wrap_text,
    TextLayoutRect, CARD_BODY_RECT, FONT_GLYPH_HEIGHT, FONT_GLYPH_WIDTH, MENU_BODY_RECT,
    OLED_WIDTH,
};

const _: () = {
    assert!(CARD_BODY_RECT.x + CARD_BODY_RECT.width <= OLED_WIDTH);
    assert!(CARD_BODY_RECT.y + CARD_BODY_RECT.height <= 114);
};

#[test]
fn rectangle_capacity_uses_glyph_metrics() {
    assert_eq!(MENU_BODY_RECT.columns(), 19);
    assert_eq!(MENU_BODY_RECT.rows(), 7);
    assert_eq!(CARD_BODY_RECT.columns(), 20);
    assert_eq!(CARD_BODY_RECT.rows(), 7);
    assert_eq!(TextLayoutRect::new(0, 0, 30, 30, 10).columns(), 5);
    assert_eq!(TextLayoutRect::new(0, 0, 30, 30, 10).rows(), 3);
    assert_eq!(
        TextLayoutRect::new(0, 0, FONT_GLYPH_WIDTH - 1, 20, 10).columns(),
        0
    );
    assert_eq!(
        TextLayoutRect::new(0, 0, 20, FONT_GLYPH_HEIGHT - 1, 10).rows(),
        0
    );
}

#[test]
fn normalization_and_wrapping_are_fixed_font_safe() {
    assert_eq!(normalize_text("  Check\n\tthe  café  "), "Check the caf?",);
    assert_eq!(
        wrap_text("abcdefghij words", 4),
        ["abcd", "efgh", "ij", "word", "s"]
    );
    assert_eq!(fit_line_ellipsis("abcdefghij", 8), "abcde...");
    assert_eq!(fit_line_ellipsis("exact", 5), "exact");
    assert_eq!(force_line_ellipsis("short", 8), "short...");
}

#[test]
fn card_layout_keeps_source_order_and_reserves_selected_action() {
    let lines = vec![
        "Setup complete".into(),
        "IP in System > Info".into(),
        "No reboot needed".into(),
        "Check the device status".into(),
        "> Close".into(),
    ];
    let rows = layout_card_body(&lines, Some(4), CARD_BODY_RECT);
    assert_eq!(
        rows.iter().map(|row| row.source_index).collect::<Vec<_>>(),
        [0, 1, 2, 3, 3, 4]
    );
    assert_eq!(rows[3].text, "Check the device");
    assert_eq!(rows[4].text, "status");
    assert_eq!(rows.last().unwrap().text, "> Close");
    assert!(rows.last().unwrap().selected);
    assert!(rows[..rows.len() - 1].iter().all(|row| !row.selected));
}

#[test]
fn card_layout_ellipsizes_only_the_final_visible_prose_row_when_lines_remain() {
    let lines = (1..=8)
        .map(|index| format!("Line {index}"))
        .chain(["> Hide".into()])
        .collect::<Vec<_>>();
    let rows = layout_card_body(&lines, Some(8), CARD_BODY_RECT);
    assert_eq!(rows.len(), CARD_BODY_RECT.rows());
    assert_eq!(rows[5].text, "Line 6...");
    assert!(rows[..5].iter().all(|row| !row.text.ends_with("...")));
    assert_eq!(rows.last().unwrap().text, "> Hide");
    assert!(rows.last().unwrap().selected);
}

#[test]
fn card_layout_moves_wrapped_content_ellipsis_to_the_final_visible_prose_row() {
    let lines = vec![
        "This is a deliberately long first line".into(),
        "Second".into(),
        "Third".into(),
        "Fourth".into(),
        "Fifth".into(),
        "Sixth".into(),
        "> Hide".into(),
    ];
    let rows = layout_card_body(&lines, Some(6), CARD_BODY_RECT);
    let prose = &rows[..rows.len() - 1];
    assert_eq!(
        prose.iter().filter(|row| row.text.ends_with("...")).count(),
        1
    );
    assert!(prose.last().unwrap().text.ends_with("..."));
    assert_eq!(rows.last().unwrap().text, "> Hide");
}
