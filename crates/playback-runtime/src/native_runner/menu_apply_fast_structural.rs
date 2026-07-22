use super::{menu_apply_fast_values::parse_indexed_key, NativeRunner};

pub(super) fn structural_draft_key(key: &str) -> bool {
    if key == "behaviorId" {
        return true;
    }
    if let Some(rest) = key.strip_prefix("instruments.") {
        return parse_indexed_key(rest)
            .is_some_and(|(_, suffix)| matches!(suffix, "type" | "mixer.route"));
    }
    if let Some(rest) = key.strip_prefix("mixer.buses.") {
        return parse_indexed_key(rest).is_some_and(|(_, suffix)| {
            matches!(suffix, "slot1.type" | "slot2.type" | "slot3.type")
        });
    }
    if let Some(rest) = key.strip_prefix("mixer.master.slots.") {
        return parse_indexed_key(rest).is_some_and(|(_, suffix)| suffix == "type");
    }
    false
}

impl NativeRunner {
    pub(super) fn commit_structural_draft_key(&mut self, key: &str) -> Result<(), String> {
        self.clear_deferred_menu_apply();
        if key == "behaviorId" {
            return self.commit_behavior_structural_draft();
        }
        if let Some(rest) = key.strip_prefix("instruments.") {
            if let Some((index, suffix)) = parse_indexed_key(rest) {
                return match suffix {
                    "type" => {
                        self.commit_instrument_type_structural_draft(index);
                        Ok(())
                    }
                    "mixer.route" => {
                        self.commit_instrument_route_structural_draft(index);
                        Ok(())
                    }
                    _ => Err(format!(
                        "unhandled structural instrument edit key `instruments.{index}.{suffix}`"
                    )),
                };
            }
        }
        if self.apply_deferred_menu_key_fast(key) {
            return Ok(());
        }
        Err(format!(
            "unhandled structural menu edit key `{key}`; add an explicit commit handler"
        ))
    }
}
