use super::{enum_item, number_item, InstrumentMenuConfig, NativeMenuItem};
use crate::native_menu::TILE_STAMP_LEVELS;

/// The `Tile ...` rows: the values Assign stamps onto the grid tiles it is pressed on.
pub(super) fn tile_stamp_items(
    config: &InstrumentMenuConfig<'_>,
    prefix: &str,
    with_level: bool,
) -> Vec<NativeMenuItem> {
    let stamp = config.tile_stamp;
    let mut items = vec![
        number_item(
            "Tile Tune",
            format!("{prefix}.tile.tuneSemis"),
            i32::from(stamp.tune_semis),
            -24,
            24,
            1,
        ),
        number_item(
            "Tile Fine",
            format!("{prefix}.tile.tuneCents"),
            i32::from(stamp.tune_cents),
            -100,
            100,
            1,
        ),
    ];
    if with_level {
        items.push(enum_item(
            "Tile Level",
            format!("{prefix}.tile.level"),
            TILE_STAMP_LEVELS.to_vec(),
            stamp.level_index.min(TILE_STAMP_LEVELS.len() - 1),
        ));
    }
    items
}
