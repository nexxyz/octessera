use super::{
    enum_item, group, mark_active, number_item, selected_index, NativeLinkLayerConfig,
    NativeMenuItem,
};
use crate::native_menu::{NativeMenuAction, NativeMenuPick, NativeMenuValue};

pub(super) fn note_mapping_group(
    layer_index: usize,
    prefix: &str,
    sense: &NativeLinkLayerConfig,
) -> NativeMenuItem {
    group(
        "Note Mapping",
        vec![
            number_item(
                "Low Note",
                format!("{prefix}.pitch.lowestNote"),
                i32::from(sense.lowest_note),
                0,
                127,
                1,
            ),
            number_item(
                "High Note",
                format!("{prefix}.pitch.highestNote"),
                i32::from(sense.highest_note),
                0,
                127,
                1,
            ),
            number_item(
                "1st Note",
                format!("{prefix}.pitch.startingNote"),
                i32::from(sense.starting_note),
                0,
                127,
                1,
            ),
            note_set_item(layer_index, prefix, &sense.scale),
            enum_item(
                "Root",
                format!("{prefix}.pitch.root"),
                platform_core::NOTE_SET_ROOTS.to_vec(),
                selected_index(platform_core::NOTE_SET_ROOTS, &sense.root),
            ),
            enum_item(
                "Out of Range",
                format!("{prefix}.pitch.outOfRange"),
                vec!["clamp", "wrap"],
                selected_index(&["clamp", "wrap"], &sense.out_of_range),
            ),
        ],
    )
}

fn note_set_item(layer_index: usize, prefix: &str, selected_id: &str) -> NativeMenuItem {
    let ids = platform_core::note_set_ids().collect::<Vec<_>>();
    let children = platform_core::note_set_categories()
        .iter()
        .map(|category| note_set_category_item(*category, layer_index))
        .collect();
    let mut item = NativeMenuItem {
        label: "Set".into(),
        breadcrumb: None,
        key: Some(format!("{prefix}.pitch.scale")),
        value: NativeMenuValue::Enum {
            options: ids.iter().map(|id| (*id).into()).collect(),
            selected: selected_index(&ids, selected_id),
        },
        children,
        pick: NativeMenuPick::Value,
    };
    mark_selected_note_set(&mut item);
    item
}

/// Points a note set picker's folders at its selected set; other items are left alone.
pub(super) fn mark_selected_note_set(item: &mut NativeMenuItem) {
    let NativeMenuValue::Enum { options, selected } = &item.value else {
        return;
    };
    if item.pick != NativeMenuPick::Value {
        return;
    }
    let Some(selected_id) = options.get(*selected).cloned() else {
        return;
    };
    mark_active(&mut item.children, &|entry| {
        matches!(
            &entry.value,
            NativeMenuValue::Action(NativeMenuAction::SelectNoteSet { note_set_id, .. })
                if *note_set_id == selected_id
        )
    });
}

fn note_set_category_item(
    category: platform_core::NoteSetCategory,
    layer_index: usize,
) -> NativeMenuItem {
    let children = std::iter::once(NativeMenuItem {
        label: "..".into(),
        breadcrumb: None,
        key: None,
        value: NativeMenuValue::Action(NativeMenuAction::NavigateBack),
        children: vec![],
        pick: NativeMenuPick::None,
    })
    .chain(
        platform_core::note_set_registry()
            .iter()
            .filter(move |note_set| note_set.category == category)
            .map(|note_set| NativeMenuItem {
                label: note_set.label.into(),
                breadcrumb: None,
                key: None,
                value: NativeMenuValue::Action(NativeMenuAction::SelectNoteSet {
                    layer_index,
                    note_set_id: note_set.id.into(),
                }),
                children: vec![],
                pick: NativeMenuPick::None,
            }),
    )
    .collect();
    NativeMenuItem {
        label: category.label().into(),
        breadcrumb: None,
        key: Some(format!("note_set.category.{}", category.id())),
        value: NativeMenuValue::Group,
        children,
        pick: NativeMenuPick::None,
    }
}
