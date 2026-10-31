use std::time::Duration;

use crate::native_menu::{NativeMenuAction, NativeMenuSnapshot};
use crate::protocol::RuntimeStoreResult;

use super::{parse_sample_action, NativeRunner, NativeSampleBrowser, NativeToast};

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
                self.show_sample_browser(
                    Some(NativeSampleBrowser {
                        instrument_slot,
                        sample_slot,
                        dir,
                        entries,
                    }),
                    instrument_slot,
                );
                self.menu.focus_active_entry_in(&format!(
                    "sample.choose:{instrument_slot}:{sample_slot}"
                ));
            }
            RuntimeStoreResult::SampleListError {
                instrument_slot,
                sample_slot,
                dir,
                message,
            } if self.sample_browser_matches(instrument_slot, sample_slot, &dir) => {
                let entries = self.unavailable_browser_entries(instrument_slot, sample_slot, &dir);
                self.show_sample_browser(
                    Some(NativeSampleBrowser {
                        instrument_slot,
                        sample_slot,
                        dir,
                        entries,
                    }),
                    instrument_slot,
                );
                self.display.toast = Some(NativeToast { message, offset: 0 });
            }
            RuntimeStoreResult::SamplePreviewStarted { path, duration_ms } => {
                let now = self.display.transients.now();
                self.display.transients.start_sample_preview(
                    path,
                    Duration::from_millis(duration_ms),
                    self.transport.transport.clone(),
                    now,
                );
            }
            _ => {}
        }
        Ok(())
    }

    /// The selected browser row when it is the file whose preview is sounding.
    pub(super) fn sounding_preview_row(&self, menu: &NativeMenuSnapshot) -> Option<usize> {
        let now = self.display.transients.now();
        let sounding = self.display.transients.sounding_sample_preview(now)?;
        let row = menu.selected_row?;
        let Some(Some(NativeMenuAction::PlatformEffect(action))) = menu.line_actions.get(row)
        else {
            return None;
        };
        let (_, _, path) = parse_sample_action(action.strip_prefix("sample.pick:")?).ok()?;
        (path.as_deref() == Some(sounding)).then_some(row)
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
