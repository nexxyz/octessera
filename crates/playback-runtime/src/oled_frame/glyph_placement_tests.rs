use super::model::*;
use super::render::render_oled_frame;
use super::tests::{base, hint, pixel};

fn slot(index: usize) -> usize {
    11 + index * 13
}

fn lit_rows(frame: &[u8], xs: std::ops::Range<usize>, ys: std::ops::Range<usize>) -> Vec<usize> {
    ys.filter(|y| xs.clone().any(|x| pixel(frame, x, *y) != 0))
        .collect()
}

fn lit_in_column(frame: &[u8], x: usize, top: usize) -> bool {
    (top - 1..=top + 9).any(|y| pixel(frame, x, y) != 0)
}

#[test]
fn breadcrumb_capitals_start_on_the_top_row() {
    let mut input = base();
    input.display.title = "/BUILD/L1".into();
    let frame = render_oled_frame(&input);
    assert_eq!(lit_rows(&frame, 0..104, 0..8), vec![0, 1, 2, 3, 4]);

    input.display.title = "/Shape/py".into();
    let frame = render_oled_frame(&input);
    assert_eq!(lit_rows(&frame, 0..104, 0..8), vec![0, 1, 2, 3, 4, 5]);
}

#[test]
fn hint_capitals_sit_on_their_rows() {
    let mut input = base();
    input.aux_hints.normal = [
        OledAuxHint {
            turn: hint("CUT"),
            click: None,
        },
        OledAuxHint {
            turn: hint("RES"),
            click: hint("MAP"),
        },
        OledAuxHint::default(),
    ];
    let frame = render_oled_frame(&input);
    assert_eq!(
        lit_rows(&frame, 9..35, 110..128),
        vec![119, 120, 121, 122, 123]
    );
    assert_eq!(
        lit_rows(&frame, 46..72, 110..121),
        vec![115, 116, 117, 118, 119]
    );
    assert_eq!(
        lit_rows(&frame, 46..72, 121..128),
        vec![123, 124, 125, 126, 127]
    );
}

/// Every row kind keeps its value, mini bar or marker in place and clips a long label at
/// least three blank columns before it.
#[test]
fn long_labels_stop_three_columns_before_the_value() {
    let long = "Spawn Interval Glider Count";
    for index in [0, 2, 5, 6] {
        let mut input = base();
        input.display.scroll = None;
        input.selected_row = Some(1);
        input.display.rows[0].as_mut().unwrap().value = "Saw".into();
        let short = render_oled_frame(&input);
        input.display.rows[index].as_mut().unwrap().label = long.into();
        input.display.lines[index] = long.into();
        let clipped = render_oled_frame(&input);
        let top = slot(index);
        let value_x = (80..124).find(|x| lit_in_column(&short, *x, top)).unwrap();
        assert!(
            (value_x - 3..value_x).all(|x| !lit_in_column(&clipped, x, top)),
            "row {index}: label within 3 px of x={value_x}"
        );
        assert!(
            (value_x - 12..value_x - 3).any(|x| lit_in_column(&clipped, x, top)),
            "row {index}: label stops short of its room"
        );
        assert!(
            (value_x..128)
                .all(|x| (top - 1..=top + 9).all(|y| pixel(&clipped, x, y) == pixel(&short, x, y))),
            "row {index}: value moved"
        );
    }
}
