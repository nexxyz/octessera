use super::{port_ids, port_position};

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

#[test]
fn port_ids_are_names_with_an_occurrence_suffix_for_duplicates() {
    assert_eq!(
        port_ids(&names(&["Synth", "Drums", "Synth"])),
        ["name:Synth", "name:Drums", "name:Synth#1"]
    );
}

#[test]
fn a_saved_port_is_found_by_name_after_the_ports_reorder() {
    let before = names(&["Synth", "Drums"]);
    let saved = port_ids(&before)[1].clone();

    let after = names(&["Drums", "Keys", "Synth"]);

    assert_eq!(port_position(&saved, &after), Some(0));
    assert_eq!(port_position("name:Synth", &after), Some(2));
}

#[test]
fn a_missing_or_list_position_id_finds_no_port() {
    let ports = names(&["Synth", "Drums"]);

    assert_eq!(port_position("name:Keys", &ports), None);
    assert_eq!(port_position("0", &ports), None);
}
