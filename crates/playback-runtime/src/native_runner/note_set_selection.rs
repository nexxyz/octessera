use super::NativeRunner;

impl NativeRunner {
    pub(super) fn select_note_set(
        &mut self,
        layer_index: usize,
        note_set_id: &str,
    ) -> Result<(), String> {
        if platform_core::note_set_by_id(note_set_id).is_none() {
            return Err(format!("unsupported note set `{note_set_id}`"));
        }
        let key = format!("layers.{layer_index}.link.pitch.scale");
        let changed = self.menu.set_enum_value_for_key(&key, note_set_id);
        if changed {
            self.apply_or_schedule_menu_key(&key)?;
        }
        if !self.menu.focus_item_key(&key) {
            return Err(format!(
                "note set menu item not found for layer {layer_index}"
            ));
        }
        Ok(())
    }
}
