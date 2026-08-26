use super::prefix_line;

#[test]
fn auto_mapped_action_rows_keep_equal_prefix_alignment() {
    assert_eq!(
        prefix_line(">!Do It".into(), Some("1!".into())),
        "> 1!Do It"
    );
    assert_eq!(prefix_line(" !Do It".into(), Some("1!".into())), "1!Do It");
}

#[test]
fn auto_mapped_value_rows_keep_turn_prefix_alignment() {
    assert_eq!(
        prefix_line("> Cutoff".into(), Some("1-".into())),
        "> 1-Cutoff"
    );
    assert_eq!(
        prefix_line("  Cutoff".into(), Some("1-".into())),
        "1-Cutoff"
    );
}
