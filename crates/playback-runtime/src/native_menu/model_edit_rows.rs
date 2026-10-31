//! Edits that reshape other rows update only those rows in place, including their copies in
//! the parameter pickers: a full menu rebuild costs hundreds of milliseconds on the boards.

use super::binding_picker::{
    axis_binding_label, current_binding_rows, labelled_binding, mark_active_binding,
};
use super::binding_picker_rows::{
    picker_rows, update_instrument_bindings, update_play_fx_bindings, update_shape_bindings,
};
use super::link::link_instrument_options;
use super::link_sections::scanning_group;
use super::model_root::{instrument_item, instrument_item_label};
use super::model_search::{
    find_item_by_key, find_item_by_key_mut, find_item_path_by_key,
    replace_group_children_containing_direct_key, replace_group_label_containing_direct_key,
    update_items,
};
use super::play::{drums_page, play_fx_page_children};
use super::section_labels::{LINK_LABEL, SHAPE_LABEL};
use super::system_aux::{click_none_key, name_click_pickers_from_menu, show_click_binding};
use super::{
    NativeLinkLayerConfig, NativeMenuAction, NativeMenuConfig, NativeMenuModel, NativeMenuValue,
    NativeParamBindingSpec,
};

impl NativeMenuModel {
    /// Shows a parameter picker's binding: its label and its Current and Range rows.
    pub fn show_param_binding(&mut self, target: &str, binding: Option<&NativeParamBindingSpec>) {
        let Some(picker) = find_item_by_key_mut(&mut self.root, target) else {
            return;
        };
        let binding = binding.map(|binding| labelled_binding(binding, &picker.children));
        let binding = binding.as_ref();
        if let Some((prefix, _)) = picker.label.split_once(": ") {
            picker.label = axis_binding_label(prefix, binding);
        }
        let rows = binding
            .map(|binding| current_binding_rows(target, binding))
            .unwrap_or_default()
            .into_iter()
            .filter_map(|row| picker_rows(row, target))
            .collect::<Vec<_>>();
        let row_keys = ["current", "rangeMin", "rangeMax"].map(|row| format!("{target}.{row}"));
        picker
            .children
            .retain(|item| !item.key.as_ref().is_some_and(|key| row_keys.contains(key)));
        let at = 1.min(picker.children.len());
        picker.children.splice(at..at, rows);
        mark_active_binding(
            &mut picker.children,
            binding.map(|binding| binding.key.as_str()),
        );
    }

    /// Shows an Aux click binding on its Clk or S+Clk picker.
    pub fn show_aux_click_binding(
        &mut self,
        index: usize,
        shifted: bool,
        action: Option<&NativeMenuAction>,
    ) {
        let mut path = Vec::new();
        if !find_item_path_by_key(&self.root, &click_none_key(index, shifted), &mut path) {
            return;
        }
        let picker = path[..path.len() - 1]
            .iter()
            .fold(&mut self.root, |node, index| &mut node.children[*index]);
        show_click_binding(picker, index, shifted, action);
        name_click_pickers_from_menu(&mut self.root);
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
            let name_key = format!("instruments.{index}.name");
            let name_cursor = match find_item_by_key(item, &name_key).map(|name| &name.value) {
                Some(NativeMenuValue::Text { cursor, .. }) => *cursor,
                _ => 0,
            };
            *item = instrument_item(config, index, label);
            if let Some(NativeMenuValue::Text { value, cursor, .. }) =
                find_item_by_key_mut(item, &name_key).map(|name| &mut name.value)
            {
                *cursor = name_cursor.min(value.len() + 1);
            }
        }
        update_instrument_bindings(&mut self.root, config, index);
        self.clamp_cursor_to_siblings();
    }

    /// The overview label shows an instrument's type and route or MIDI channel.
    pub fn show_instrument_label(&mut self, config: &NativeMenuConfig, index: usize) {
        if let Some(label) = config.instrument_labels.get(index) {
            replace_group_label_containing_direct_key(
                &mut self.root,
                &format!("instruments.{index}.type"),
                &instrument_item_label(config, index, label),
            );
        }
    }

    /// Refreshes the parameter-picker copies of a Shape group (`FX Buses`, `Global FX`) after
    /// its own rows were updated in place.
    pub fn show_shape_group_bindings(&mut self, label: &str) {
        let source = self
            .root
            .children
            .iter()
            .find(|item| item.label == SHAPE_LABEL)
            .and_then(|shape| shape.children.iter().find(|item| item.label == label))
            .cloned();
        if let Some(source) = source {
            update_shape_bindings(&mut self.root, &source);
        }
    }

    /// An instrument's name and type also show in the Link instrument choices, the Aux
    /// Sample Assign rows and the Play Drums page.
    pub fn show_instrument_identity(&mut self, config: &NativeMenuConfig, index: usize) {
        self.show_instrument_rows(config, index);
        let options = link_instrument_options(&config.instrument_labels);
        let sample_assign = format!(".click.sample.{index}");
        let label = config.instrument_labels.get(index);
        if let Some(link) = self
            .root
            .children
            .iter_mut()
            .find(|item| item.label == LINK_LABEL)
        {
            update_items(link, &mut |item| {
                let Some(key) = item.key.as_deref() else {
                    return;
                };
                if key.ends_with(&sample_assign) {
                    if let Some(label) = label {
                        item.label.clone_from(label);
                    }
                } else if key.starts_with("layers.")
                    && key.contains(".link.mapping.")
                    && key.ends_with(".slot")
                {
                    if let NativeMenuValue::Enum { options: shown, .. } = &mut item.value {
                        shown.clone_from(&options);
                    }
                }
            });
        }
        if let Some(drums) = find_item_by_key_mut(&mut self.root, "play.page.drums") {
            *drums = drums_page(config);
        }
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

    /// Rows replaced in place can remove the page that is open; then `key` takes the focus.
    pub fn keep_focus_on_open_page(&mut self, key: &str) {
        let mut node = &self.root;
        for index in &self.state.stack {
            match node.children.get(*index) {
                Some(next) if !next.children.is_empty() => node = next,
                _ => {
                    self.focus_item_key(key);
                    return;
                }
            }
        }
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
