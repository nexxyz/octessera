use super::NativeRunnerOutbox;

impl NativeRunnerOutbox {
    pub(super) fn resolve_momentary_start_epoch(&mut self, requested: u64) -> u64 {
        if !self.momentary_epoch_wrapped
            && requested != 0
            && requested != u64::MAX
            && requested > self.momentary_epoch
            && !self
                .active_momentary_epochs
                .values()
                .any(|&epoch| epoch == requested)
        {
            self.momentary_epoch = requested;
            requested
        } else {
            self.allocate_momentary_epoch()
        }
    }

    fn allocate_momentary_epoch(&mut self) -> u64 {
        let mut candidate = self.momentary_epoch.wrapping_add(1);
        if candidate == 0 || candidate == u64::MAX {
            candidate = 1;
            self.momentary_epoch_wrapped = true;
        }
        for _ in 0..=self.active_momentary_epochs.len() {
            if !self
                .active_momentary_epochs
                .values()
                .any(|&epoch| epoch == candidate)
            {
                self.momentary_epoch = candidate;
                return candidate;
            }
            candidate = candidate.wrapping_add(1);
            if candidate == 0 || candidate == u64::MAX {
                candidate = 1;
                self.momentary_epoch_wrapped = true;
            }
        }
        unreachable!("momentary epoch space exhausted by active effects")
    }
}
