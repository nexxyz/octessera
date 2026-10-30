use super::*;

fn instrument_page(cfg: NativeMenuConfig) -> (String, String) {
    let mut menu = NativeMenuModel::new(cfg);
    assert!(menu.focus_item_key("instruments.0.type"));
    let row = menu
        .root
        .children
        .iter()
        .find(|item| item.label == "Shape")
        .and_then(|shape| shape.children.first())
        .and_then(|instruments| instruments.children.first())
        .map(|item| item.label.clone())
        .unwrap();
    (row, menu.snapshot().path)
}

#[test]
pub(crate) fn instrument_page_breadcrumb_uses_the_short_model_name() {
    assert_eq!(
        instrument_page(config()),
        ("I1: synth direct".into(), "../I1: synth".into())
    );

    let mut midi = config();
    midi.instrument_types[0] = "midi".into();
    midi.instrument_midi_channels[0] = 3;
    assert_eq!(
        instrument_page(midi),
        ("I1: midi ch3".into(), "../I1: midi".into())
    );

    let mut none = config();
    none.instrument_types[0] = "none".into();
    assert_eq!(
        instrument_page(none),
        ("I1: none".into(), "../I1: none".into())
    );
}
