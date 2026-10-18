use super::{enum_item, number_item, InstrumentMenuConfig, NativeMenuItem};
use crate::native_menu::CELL_STAMP_LEVELS;

/// The `Cell ...` rows: the values Assign stamps onto the grid cells it is pressed on.
pub(super) fn cell_stamp_items(
    config: &InstrumentMenuConfig<'_>,
    prefix: &str,
    with_level: bool,
) -> Vec<NativeMenuItem> {
    let stamp = config.cell_stamp;
    let mut items = vec![
        number_item(
            "Cell Tune",
            format!("{prefix}.cell.tuneSemis"),
            i32::from(stamp.tune_semis),
            -24,
            24,
            1,
        ),
        number_item(
            "Cell Fine",
            format!("{prefix}.cell.tuneCents"),
            i32::from(stamp.tune_cents),
            -100,
            100,
            1,
        ),
    ];
    if with_level {
        items.push(enum_item(
            "Cell Level",
            format!("{prefix}.cell.level"),
            CELL_STAMP_LEVELS.to_vec(),
            stamp.level_index.min(CELL_STAMP_LEVELS.len() - 1),
        ));
    }
    items
}
