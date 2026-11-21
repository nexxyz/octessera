use super::NativeRunner;

impl NativeRunner {
    /// A manual edit of these behavior settings owns them from now on: the sources holding
    /// them let go.
    pub(super) fn yield_behavior_config_fields_to_edit(
        &mut self,
        layer_index: usize,
        fields: &[String],
    ) {
        for field in fields {
            let key = format!("layers.{layer_index}.build.behaviorConfig.{field}");
            self.modulation_process.yield_key_to_edit(&key);
        }
    }
}
