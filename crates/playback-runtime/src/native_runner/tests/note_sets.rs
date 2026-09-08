use super::*;
use crate::native_menu::{NativeMenuAction, NativeMenuValue};

const NOTE_SET_KEY: &str = "layers.0.pulses.pitch.scale";

#[test]
pub(crate) fn note_set_parent_is_a_bindable_flat_enum_with_four_folders() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let item = runner.menu.item_for_key(NOTE_SET_KEY).unwrap();
    let NativeMenuValue::Enum { options, .. } = item.value else {
        panic!("note set parent should be an enum");
    };
    let expected_options = platform_core::note_set_ids()
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert_eq!(options, expected_options);
    assert_eq!(item.children.len(), 4);
    assert!(item.children.iter().all(|category| category
        .children
        .first()
        .is_some_and(|child| child.label == "..")));
    assert_eq!(
        item.children
            .iter()
            .map(|child| child.label.as_str())
            .collect::<Vec<_>>(),
        ["Scales", "Chord Tones", "Pentatonic", "Symmetric"]
    );
    assert_eq!(
        item.children
            .iter()
            .map(|child| child.key.as_deref())
            .collect::<Vec<_>>(),
        [
            Some("note_set.category.scales"),
            Some("note_set.category.chord_tones"),
            Some("note_set.category.pentatonic"),
            Some("note_set.category.symmetric"),
        ]
    );
    let binding = runner.menu.binding_spec_for_key(NOTE_SET_KEY).unwrap();
    assert_eq!(binding.kind, "enum");
    assert_eq!(binding.options, expected_options);
}

#[test]
pub(crate) fn host_device_input_navigates_and_selects_every_note_set() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for note_set in platform_core::note_set_registry() {
        assert!(runner.menu.focus_item_key(NOTE_SET_KEY));
        send_main(&mut runner, "encoder_press", None);
        let category_index = platform_core::note_set_categories()
            .iter()
            .position(|category| *category == note_set.category)
            .unwrap();
        send_turn(&mut runner, category_index as i8);
        assert_eq!(runner.menu.current_label(), Some(note_set.category.label()));
        send_main(&mut runner, "encoder_press", None);
        assert_eq!(runner.menu.current_label(), Some(".."));
        let leaf_index = platform_core::note_set_registry()
            .iter()
            .filter(|candidate| candidate.category == note_set.category)
            .position(|candidate| candidate.id == note_set.id)
            .unwrap();
        let leaf_snapshot = send_turn(&mut runner, (leaf_index + 1) as i8);
        let leaf_row = leaf_snapshot["display"]["lines"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .find(|line| line.contains(note_set.label))
            .unwrap();
        assert!(leaf_row.starts_with(">!"));
        assert!(leaf_row.chars().count() <= 19);
        assert!(!leaf_row.contains("..."));
        for line in leaf_snapshot["display"]["lines"].as_array().unwrap() {
            let line = line.as_str().unwrap();
            assert!(line.chars().count() <= 19, "{line:?}");
            assert!(!line.contains("..."), "{line:?}");
        }
        assert!(leaf_snapshot["display"]["lines"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .any(|line| line.starts_with(" !")));
        let snapshot = send_main(&mut runner, "encoder_press", None);
        assert_eq!(runner.pulses_layers[0].scale, note_set.id);
        assert_eq!(runner.menu.current_key(), Some(NOTE_SET_KEY));
        assert!(runner
            .menu
            .current_focus_path()
            .ends_with("Note Mapping > Set"));
        let parent = snapshot["display"]["lines"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .find(|line| line.contains("Set:"))
            .unwrap();
        assert_eq!(parent, &format!("> Set: {} >", note_set.compact_label));
        assert!(parent.chars().count() <= 19);
        assert!(!parent.contains("..."));
    }
}

#[test]
pub(crate) fn physical_back_from_categories_returns_to_note_mapping_set() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key(NOTE_SET_KEY));
    send_main(&mut runner, "encoder_press", None);
    send_main(&mut runner, "button_a", Some(true));
    assert_eq!(runner.menu.current_key(), Some(NOTE_SET_KEY));
    assert!(runner
        .menu
        .current_focus_path()
        .ends_with("Note Mapping > Set"));
}

#[test]
pub(crate) fn main_on_category_back_row_returns_one_level() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key(NOTE_SET_KEY));
    send_main(&mut runner, "encoder_press", None);
    send_main(&mut runner, "encoder_press", None);
    assert_eq!(runner.menu.current_label(), Some(".."));
    send_main(&mut runner, "encoder_press", None);
    assert_eq!(runner.menu.current_label(), Some("Scales"));
}

#[test]
pub(crate) fn physical_back_on_category_back_row_returns_one_level() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key(NOTE_SET_KEY));
    send_main(&mut runner, "encoder_press", None);
    send_main(&mut runner, "encoder_press", None);
    assert_eq!(runner.menu.current_label(), Some(".."));
    send_main(&mut runner, "button_a", Some(true));
    assert_eq!(runner.menu.current_label(), Some("Scales"));
}

#[test]
pub(crate) fn selecting_the_current_note_set_does_not_dirty_config() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let before = runner.config_revision;
    runner
        .execute_menu_action(NativeMenuAction::SelectNoteSet {
            layer_index: 0,
            note_set_id: runner.pulses_layers[0].scale.clone(),
        })
        .unwrap();
    assert_eq!(runner.config_revision, before);
    assert!(!runner.config_dirty);
    assert_eq!(runner.menu.current_key(), Some(NOTE_SET_KEY));
}

#[test]
pub(crate) fn note_set_parent_selected_and_unselected_rows_are_exact() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key(NOTE_SET_KEY));
    let selected = runner.menu.snapshot().lines;
    let selected = selected
        .iter()
        .find(|line| line.starts_with("> Set:"))
        .unwrap();
    assert_eq!(selected, "> Set: Maj Pent >");
    assert!(selected.chars().count() <= 19);
    assert!(!selected.contains("..."));

    assert!(runner.menu.focus_item_key("layers.0.pulses.pitch.root"));
    let unselected = runner.menu.snapshot().lines;
    let unselected = unselected
        .iter()
        .find(|line| line.starts_with("  Set:"))
        .unwrap();
    assert_eq!(unselected, "  Set: Maj Pent >");
    assert!(unselected.chars().count() <= 19);
    assert!(!unselected.contains("..."));
}

#[test]
pub(crate) fn fn_aux_binds_note_set_parent_but_not_note_set_leaf() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key(NOTE_SET_KEY));
    runner.display.ui.fn_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "aux1" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(
        runner.aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        Some(NOTE_SET_KEY)
    );
    assert!(runner.aux_bindings[0]
        .as_ref()
        .is_some_and(|binding| binding.press_action.is_none()));

    runner.aux_bindings[1] = Some(NativeAuxBinding {
        turn_key: Some("masterVolume".into()),
        press_action: Some(NativeMenuAction::ResetBehavior),
    });
    runner.display.ui.fn_held = false;
    assert!(runner.menu.focus_item_key(NOTE_SET_KEY));
    send_main(&mut runner, "encoder_press", None);
    send_main(&mut runner, "encoder_press", None);
    send_turn(&mut runner, 1);
    assert_eq!(runner.menu.current_label(), Some("Chromatic"));
    runner.display.ui.fn_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "aux2" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(
        runner.aux_bindings[1]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        Some("masterVolume")
    );
    assert!(matches!(
        runner.aux_bindings[1]
            .as_ref()
            .and_then(|binding| binding.press_action.as_ref()),
        Some(NativeMenuAction::ResetBehavior)
    ));
}

fn send_turn(runner: &mut NativeRunner, delta: i8) -> Value {
    snapshot_from(&send(
        runner,
        json!({"type": "encoder_turn", "id": "main", "delta": delta}),
    ))
}

fn send_main(runner: &mut NativeRunner, input_type: &str, pressed: Option<bool>) -> Value {
    let mut input = json!({"type": input_type, "id": "main"});
    if let Some(pressed) = pressed {
        input["pressed"] = json!(pressed);
    }
    snapshot_from(&send(runner, input))
}

fn send(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: Some(true),
        })
        .unwrap()
}
