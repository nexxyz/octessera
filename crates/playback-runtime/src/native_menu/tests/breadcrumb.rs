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
        ("I1: Synth direct".into(), "/S/Instruments/I1: Synth".into())
    );

    let mut midi = config();
    midi.instrument_types[0] = "midi".into();
    midi.instrument_midi_channels[0] = 3;
    assert_eq!(
        instrument_page(midi),
        (
            "I1: MIDI Inst. ch3".into(),
            "../Instruments/I1: MIDI Inst.".into()
        )
    );

    let mut none = config();
    none.instrument_types[0] = "none".into();
    assert_eq!(
        instrument_page(none),
        ("I1: None".into(), "/S/Instruments/I1: None".into())
    );
}

#[test]
pub(crate) fn fader_rows_read_fader_n_and_the_breadcrumb_stays_short() {
    let mut menu = NativeMenuModel::new(config());
    assert!(menu.focus_item_key("play.faders.0.target"));
    assert_eq!(menu.snapshot().path, "/P/Faders/F1");
    assert!(menu.focus_item_key("play.faders.7.target"));
    assert_eq!(menu.snapshot().path, "/P/Faders/F8");
    let faders = menu.root.children.iter().find(|item| item.label == "Play");
    let rows: Vec<_> = faders
        .and_then(|play| play.children.iter().find(|item| item.label == "Faders"))
        .map(|group| {
            group
                .children
                .iter()
                .take(4)
                .map(|i| i.label.as_str())
                .collect()
        })
        .unwrap();
    assert_eq!(rows, ["Action Cols", "Tile Map", "Fader 1", "Fader 2"]);
}
