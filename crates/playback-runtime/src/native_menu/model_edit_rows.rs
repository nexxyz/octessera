//! Edits that reshape other rows update only those rows in place, including their copies in
//! the parameter pickers: a full menu rebuild costs hundreds of milliseconds on the boards.

use super::binding_picker::current_binding_rows;
use super::binding_picker_rows::{update_instrument_bindings, update_play_fx_bindings};
use super::link::link_instrument_options;
use super::link_sections::scanning_group;
use super::model_root::instrument_item;
use super::model_search::{find_item_by_key_mut, replace_group_children_containing_direct_key};
use super::play::play_fx_page_children;
use super::section_labels::SHAPE_LABEL;
use super::{NativeLinkLayerConfig, NativeMenuConfig, NativeMenuModel, NativeParamBindingSpec};

impl NativeMenuModel {
    pub fn show_param_binding_rows(&mut self, target: &str, binding: &NativeParamBindingSpec) {
        let Some(picker) = find_item_by_key_mut(&mut self.root, target) else {
            return;
        };
        for row in current_binding_rows(target, binding) {
            if let Some(slot) = picker.children.iter_mut().find(|item| item.key == row.key) {
                *slot = row;
            }
        }
    }

    pub fn show_play_fx_rows(&mut self, config: &NativeMenuConfig) {
        if let Some(page) = find_item_by_key_mut(&mut self.root, "play.page.fx") {
            page.children = play_fx_page_children(config);
        }
        update_play_fx_bindings(&mut self.root, config);
        self.clamp_cursor_to_siblings();
    }

    pub fn show_instrument_rows(&mut self, config: &NativeMenuConfig, index: usize) {
        let item = self
            .root
            .children
            .iter_mut()
            .find(|item| item.label == SHAPE_LABEL)
            .and_then(|shape| {
                shape
                    .children
                    .iter_mut()
                    .find(|item| item.label == "Instruments")
            })
            .and_then(|instruments| instruments.children.get_mut(index));
        if let (Some(item), Some(label)) = (item, config.instrument_labels.get(index)) {
            *item = instrument_item(config, index, label);
        }
        update_instrument_bindings(&mut self.root, config, index);
        self.clamp_cursor_to_siblings();
    }

    pub fn show_link_scanning_rows(
        &mut self,
        layer_index: usize,
        sense: &NativeLinkLayerConfig,
        instrument_labels: &[String],
    ) {
        let prefix = format!("layers.{layer_index}.link");
        let scanning = scanning_group(&prefix, sense, &link_instrument_options(instrument_labels));
        replace_group_children_containing_direct_key(
            &mut self.root,
            &format!("{prefix}.scanMode"),
            &scanning.children,
        );
        self.clamp_cursor_to_siblings();
    }

    pub fn keep_focus_on_key(&mut self, key: &str) {
        if self.current_key() == Some(key) {
            return;
        }
        let editing = self.state.editing;
        if self.focus_item_key(key) {
            self.state.editing = editing;
        }
    }
}
