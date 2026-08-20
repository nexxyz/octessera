use crate::protocol::RuntimeStoreResult;

use super::{NativeRunner, NativeSampleBrowser, NativeToast};

impl NativeRunner {
    pub(super) fn apply_sample_browser_result(
        &mut self,
        result: RuntimeStoreResult,
    ) -> Result<(), String> {
        match result {
            RuntimeStoreResult::SampleListResult {
                instrument_slot,
                sample_slot,
                dir,
                entries,
            } if self.sample_browser_matches(instrument_slot, sample_slot, &dir) => {
                let entries =
                    self.browser_entries_for_result(instrument_slot, sample_slot, &dir, entries);
                self.sample_browser = Some(NativeSampleBrowser {
                    instrument_slot,
                    sample_slot,
                    dir,
                    entries,
                });
                self.menu.rebuild(self.menu_config());
            }
            RuntimeStoreResult::SampleListError {
                instrument_slot,
                sample_slot,
                dir,
                message,
            } if self.sample_browser_matches(instrument_slot, sample_slot, &dir) => {
                let entries = self.unavailable_browser_entries(instrument_slot, sample_slot, &dir);
                self.sample_browser = Some(NativeSampleBrowser {
                    instrument_slot,
                    sample_slot,
                    dir,
                    entries,
                });
                self.display.toast = Some(NativeToast { message, offset: 0 });
                self.menu.rebuild(self.menu_config());
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn sample_browser_matches(
        &self,
        instrument_slot: usize,
        sample_slot: usize,
        dir: &str,
    ) -> bool {
        self.sample_browser.as_ref().is_some_and(|browser| {
            browser.instrument_slot == instrument_slot
                && browser.sample_slot == sample_slot
                && browser.dir == dir
        })
    }
}
