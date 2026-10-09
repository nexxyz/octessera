use super::*;
use crate::native_menu::NativeMenuItem;

fn item_paths(node: &NativeMenuItem, prefix: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
    for (index, child) in node.children.iter().enumerate() {
        prefix.push(index);
        out.push(prefix.clone());
        item_paths(child, prefix, out);
        prefix.pop();
    }
}

#[test]
fn selected_item_snapshot_presents_the_same_selected_line_as_the_full_menu() {
    for jack_audio_required in [false, true] {
        let mut runner = NativeRunner::new(NativeRunnerConfig {
            jack_audio_required,
            ..NativeRunnerConfig::default()
        })
        .unwrap();
        let mut paths = Vec::new();
        item_paths(&runner.menu.root, &mut Vec::new(), &mut paths);
        assert!(paths.len() > 100);
        for path in paths {
            for editing in [false, true] {
                runner.menu.state.stack = path[..path.len() - 1].to_vec();
                runner.menu.state.cursor = path[path.len() - 1];
                runner.menu.state.editing = editing;
                let full = runner.menu.snapshot();
                let selected = runner.menu.selected_item_snapshot();
                assert_eq!(
                    super::super::snapshot_display::selected_menu_presentation_line(&runner, &full),
                    super::super::snapshot_display::selected_menu_presentation_line(
                        &runner, &selected
                    ),
                    "{path:?} editing={editing}"
                );
                let key = |snapshot: &crate::native_menu::NativeMenuSnapshot| {
                    snapshot
                        .selected_row
                        .and_then(|row| snapshot.line_keys.get(row).cloned())
                };
                assert_eq!(key(&full), key(&selected), "{path:?} editing={editing}");
            }
        }
    }
}
