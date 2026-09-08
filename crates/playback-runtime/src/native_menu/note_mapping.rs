use super::{
    enum_item, group, number_item, selected_index, NativeMenuItem, NativePulsesLayerConfig,
};
use crate::native_menu::{NativeMenuAction, NativeMenuValue};

pub(super) fn note_mapping_group(
    layer_index: usize,
    prefix: &str,
    sense: &NativePulsesLayerConfig,
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
                "Start Note",
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
    NativeMenuItem {
        label: "Set".into(),
        key: Some(format!("{prefix}.pitch.scale")),
        value: NativeMenuValue::Enum {
            options: ids.iter().map(|id| (*id).into()).collect(),
            selected: selected_index(&ids, selected_id),
        },
        children,
    }
}

fn note_set_category_item(
    category: platform_core::NoteSetCategory,
    layer_index: usize,
) -> NativeMenuItem {
    let children = std::iter::once(NativeMenuItem {
        label: "..".into(),
        key: None,
        value: NativeMenuValue::Action(NativeMenuAction::NavigateBack),
        children: vec![],
    })
    .chain(
        platform_core::note_set_registry()
            .iter()
            .filter(move |note_set| note_set.category == category)
            .map(|note_set| NativeMenuItem {
                label: note_set.label.into(),
                key: None,
                value: NativeMenuValue::Action(NativeMenuAction::SelectNoteSet {
                    layer_index,
                    note_set_id: note_set.id.into(),
                }),
                children: vec![],
            }),
    )
    .collect();
    NativeMenuItem {
        label: category.label().into(),
        key: Some(format!("note_set.category.{}", category.id())),
        value: NativeMenuValue::Group,
        children,
    }
}
