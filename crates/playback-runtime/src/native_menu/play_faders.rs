use super::binding_picker::parameter_picker_group;
use super::binding_picker_rows::name_parameter_pickers;
use super::model_search::{find_item_by_key_mut, find_item_path_by_key};
use super::system_aux::{click_picker_group, show_click_binding, ClickPickerSlot};
use super::{
    action_item, bool_item, enum_item, group, keyed_group, number_item, selected_index,
    NativeMenuAction, NativeMenuConfig, NativeMenuItem, NativeMenuModel,
};
use crate::native_runner::play_pages::faders::FADER_SIDE_FIELDS;

/// The Tile Map's parameter picker; a param tile steps its key like an Aux turn.
pub(crate) const FADER_TILE_TARGET: &str = "play.faders.tile.target";
pub(crate) const FADER_TILE_MAP_ACTION: &str = "play.faders.tile.map";
pub(crate) const FADER_TILE_TYPES: [&str; 3] = ["none", "action", "param"];
const TILE_MAP_KEY: &str = "play.faders.tile";

/// The row labels of a Faders column's two sides: the plain press and the Shift + press target.
const SIDE_LABELS: [(&str, &str); 2] = [("Target", "Invert"), ("S+Target", "S+Invert")];

pub(super) fn faders_page_children(config: &NativeMenuConfig) -> Vec<NativeMenuItem> {
    let mut children = config
        .faders
        .targets
        .iter()
        .enumerate()
        .map(|(column, targets)| {
            let rows = SIDE_LABELS
                .iter()
                .zip(&FADER_SIDE_FIELDS)
                .zip(targets)
                .flat_map(|(((label, invert_label), fields), target)| {
                    [
                        parameter_picker_group(
                            label,
                            format!("play.faders.{column}.{}", fields.target),
                            target.as_ref(),
                        ),
                        bool_item(
                            *invert_label,
                            format!("play.faders.{column}.{}", fields.invert),
                            target.as_ref().is_some_and(|binding| binding.invert),
                        ),
                    ]
                })
                .collect();
            let mut fader = group(format!("Fader {}", column + 1), rows);
            fader.breadcrumb = Some(format!("F{}", column + 1));
            fader
        })
        .collect::<Vec<_>>();
    children.insert(
        0,
        number_item(
            "Action Cols",
            "play.faders.actionCols",
            i32::from(config.faders.action_cols),
            0,
            8,
            1,
        ),
    );
    children.insert(
        1,
        keyed_group("Tile Map", TILE_MAP_KEY, tile_map_children(config)),
    );
    children
}

/// Type first; then the action pickers or the parameter picker the type needs; then Map to Grid.
fn tile_map_children(config: &NativeMenuConfig) -> Vec<NativeMenuItem> {
    let faders = &config.faders;
    let mut children = vec![enum_item(
        "Type",
        "play.faders.tile.type",
        FADER_TILE_TYPES.to_vec(),
        selected_index(&FADER_TILE_TYPES, &faders.tile_type),
    )];
    match faders.tile_type.as_str() {
        "action" => {
            for (shifted, action) in [false, true].into_iter().zip(&faders.tile_actions) {
                let slot = ClickPickerSlot::FaderTile { shifted };
                children.push(click_picker_group(slot, action.as_ref(), config));
            }
        }
        "param" => children.push(parameter_picker_group(
            "Target",
            FADER_TILE_TARGET.into(),
            faders.tile_target.as_ref(),
        )),
        _ => {}
    }
    children.push(action_item(
        "Map to Grid",
        FADER_TILE_MAP_ACTION,
        NativeMenuAction::PlatformEffect(FADER_TILE_MAP_ACTION.into()),
    ));
    children
}

impl NativeMenuModel {
    /// Rebuilds the Tile Map rows in place after a Type change.
    pub fn show_fader_tile_rows(&mut self, config: &NativeMenuConfig) {
        if let Some(tile_map) = find_item_by_key_mut(&mut self.root, TILE_MAP_KEY) {
            tile_map.children = tile_map_children(config);
            name_parameter_pickers(tile_map, &self.parameter_tree);
        }
        self.sync_open_picker();
        self.clamp_cursor_to_siblings();
    }

    /// Shows a Tile Map Action or S+Action binding on its picker.
    pub fn show_fader_tile_action(&mut self, shifted: bool, action: Option<&NativeMenuAction>) {
        let slot = ClickPickerSlot::FaderTile { shifted };
        let none_key = if shifted {
            "play.faders.tile.shiftAction.none"
        } else {
            "play.faders.tile.action.none"
        };
        let mut path = Vec::new();
        if !find_item_path_by_key(&self.root, none_key, &mut path) {
            return;
        }
        let picker = path[..path.len() - 1]
            .iter()
            .fold(&mut self.root, |node, index| &mut node.children[*index]);
        show_click_binding(picker, slot, action);
    }
}
