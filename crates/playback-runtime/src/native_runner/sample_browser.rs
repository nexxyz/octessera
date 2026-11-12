use crate::native_menu::{
    sample_display_name, NativeMenuAction, NativeSampleAvailability, PlatformAction,
};
use crate::protocol::{RuntimeAudioCommand, RuntimePlatformEffect, SampleEntry};

use super::{
    parent_dir, parse_sample_choose_slots, same_sample_path, NativeRunner, NativeSampleBrowser,
};

mod sample_availability;

impl NativeRunner {
    /// Opens the browser at `dir`; from a slot's loaded-sample row it opens that slot's browse
    /// list too.
    pub(super) fn open_sample_browser(
        &mut self,
        instrument_slot: usize,
        sample_slot: usize,
        dir: String,
    ) -> RuntimePlatformEffect {
        let from_loaded_row = self
            .menu
            .current_key()
            .is_some_and(|key| key.starts_with("sample.loaded:"));
        let effect = self.browse_sample_dir(instrument_slot, sample_slot, dir);
        if from_loaded_row {
            self.menu
                .open_item_key(&format!("sample.choose:{instrument_slot}:{sample_slot}"));
        }
        effect
    }

    /// Shows the browser at `dir`, empty until the host lists it.
    pub(super) fn browse_sample_dir(
        &mut self,
        instrument_slot: usize,
        sample_slot: usize,
        dir: String,
    ) -> RuntimePlatformEffect {
        self.show_sample_browser(
            Some(NativeSampleBrowser {
                instrument_slot,
                sample_slot,
                dir: dir.clone(),
                entries: vec![],
            }),
            instrument_slot,
        );
        RuntimePlatformEffect::SampleListRequest {
            instrument_slot,
            sample_slot,
            dir,
        }
    }

    pub(super) fn browse_sample_parent_dir(
        &mut self,
        instrument_slot: usize,
        sample_slot: usize,
    ) -> RuntimePlatformEffect {
        let dir = self
            .sample_browser
            .as_ref()
            .map(|browser| parent_dir(&browser.dir))
            .unwrap_or_default();
        self.browse_sample_dir(instrument_slot, sample_slot, dir)
    }

    pub(super) fn pick_sample(&mut self, instrument_slot: usize, sample_slot: usize, path: String) {
        let repicked_missing = self
            .current_sample_path(instrument_slot, sample_slot)
            .is_some_and(|current| same_sample_path(current, &path))
            && self
                .sample_availability
                .get(instrument_slot)
                .and_then(|slots| slots.get(sample_slot))
                == Some(&NativeSampleAvailability::Unavailable);
        let mut changed = false;
        if let Some(instrument) = self.instruments.get_mut(instrument_slot) {
            if sample_slot < instrument.sample_paths.len() {
                instrument.sample_paths[sample_slot] = Some(path);
                if let Some(availability) = self
                    .sample_availability
                    .get_mut(instrument_slot)
                    .and_then(|slots| slots.get_mut(sample_slot))
                {
                    *availability = if repicked_missing {
                        NativeSampleAvailability::Unknown
                    } else {
                        NativeSampleAvailability::Available
                    };
                }
                changed = true;
            }
        }
        if changed {
            self.queue_instrument_slot_config(instrument_slot);
            self.mark_fast_autosave_dirty();
            self.show_sample_browser(None, instrument_slot);
            let _ = self
                .menu
                .focus_item_key(&format!("sample.choose:{instrument_slot}:{sample_slot}"));
        }
    }

    pub(super) fn show_sample_browser(
        &mut self,
        browser: Option<NativeSampleBrowser>,
        instrument_slot: usize,
    ) {
        let previous = std::mem::replace(&mut self.sample_browser, browser)
            .map(|previous| previous.instrument_slot)
            .filter(|previous| *previous != instrument_slot);
        let slots = previous
            .into_iter()
            .chain([instrument_slot])
            .collect::<Vec<_>>();
        self.show_instruments_rows(&slots);
    }

    pub(super) fn browser_entries_for_result(
        &mut self,
        instrument_slot: usize,
        sample_slot: usize,
        dir: &str,
        mut entries: Vec<SampleEntry>,
    ) -> Vec<SampleEntry> {
        let Some(path) = self
            .current_sample_path(instrument_slot, sample_slot)
            .map(str::to_string)
        else {
            return entries;
        };
        if parent_dir(&path) != dir {
            return entries;
        }
        let unavailable = self
            .sample_availability
            .get(instrument_slot)
            .and_then(|slots| slots.get(sample_slot))
            == Some(&NativeSampleAvailability::Unavailable);
        if let Some(entry) = entries
            .iter_mut()
            .find(|entry| !entry.is_dir && same_sample_path(&entry.path, &path))
        {
            if unavailable {
                entry.name = sample_display_name(&path, NativeSampleAvailability::Unavailable);
            } else {
                self.mark_sample_available(instrument_slot, sample_slot);
            }
        } else {
            self.mark_sample_unavailable(instrument_slot, sample_slot);
            entries.push(SampleEntry {
                name: sample_display_name(&path, NativeSampleAvailability::Unavailable),
                path,
                is_dir: false,
            });
            sort_sample_entries(&mut entries);
        }
        entries
    }

    pub(super) fn unavailable_browser_entries(
        &mut self,
        instrument_slot: usize,
        sample_slot: usize,
        dir: &str,
    ) -> Vec<SampleEntry> {
        let Some(path) = self
            .current_sample_path(instrument_slot, sample_slot)
            .map(str::to_string)
        else {
            return vec![];
        };
        if parent_dir(&path) != dir {
            return vec![];
        }
        self.mark_sample_unavailable(instrument_slot, sample_slot);
        vec![SampleEntry {
            name: sample_display_name(&path, NativeSampleAvailability::Unavailable),
            path,
            is_dir: false,
        }]
    }

    fn current_sample_path(&self, instrument_slot: usize, sample_slot: usize) -> Option<&str> {
        self.instruments
            .get(instrument_slot)
            .and_then(|instrument| instrument.sample_paths.get(sample_slot))
            .and_then(Option::as_deref)
    }

    pub(super) fn toggle_sample_favourite(
        &mut self,
        instrument_slot: usize,
        sample_slot: usize,
        set: bool,
    ) -> Result<Option<RuntimePlatformEffect>, String> {
        let Some(browser) = self.sample_browser.as_ref() else {
            return Ok(None);
        };
        if browser.instrument_slot != instrument_slot || browser.sample_slot != sample_slot {
            return Ok(None);
        }
        let dir = browser.dir.clone();
        if set {
            if !self.sample_favourite_dirs.iter().any(|entry| entry == &dir) {
                self.sample_favourite_dirs.push(dir);
                self.mark_system_dirty();
                self.save_system_change();
            }
            self.show_toast("Favorite set");
        } else if let Some(index) = self
            .sample_favourite_dirs
            .iter()
            .position(|entry| entry == &dir)
        {
            self.sample_favourite_dirs.remove(index);
            self.mark_system_dirty();
            self.save_system_change();
            self.show_toast("Favorite removed");
        } else {
            return Ok(None);
        }
        let browser = self.sample_browser.take();
        self.show_sample_browser(browser, instrument_slot);
        Ok(None)
    }

    pub(super) fn sample_open_effect_for_current_group(&mut self) -> Option<RuntimePlatformEffect> {
        let key = self
            .menu
            .current_key()?
            .strip_prefix("sample.choose:")?
            .to_string();
        self.sample_open_effect_for_key(&key)
    }

    pub(super) fn sample_open_effect_for_key(
        &mut self,
        key: &str,
    ) -> Option<RuntimePlatformEffect> {
        let key = key.strip_prefix("sample.choose:").unwrap_or(key);
        let (instrument_slot, sample_slot) = parse_sample_choose_slots(key)?;
        let dir = self
            .sample_browser
            .as_ref()
            .filter(|browser| {
                browser.instrument_slot == instrument_slot && browser.sample_slot == sample_slot
            })
            .map(|browser| browser.dir.clone())
            .unwrap_or_default();
        Some(self.browse_sample_dir(instrument_slot, sample_slot, dir))
    }

    pub(super) fn preview_selected_sample(&self) -> Result<Option<RuntimePlatformEffect>, String> {
        let Some(NativeMenuAction::PlatformEffect(PlatformAction::SamplePick {
            instrument_slot,
            sample_slot,
            path,
        })) = self.menu.snapshot().selected_action
        else {
            return Ok(None);
        };
        Ok(Some(RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::SamplePreview {
                instrument_slot,
                sample_slot,
                path,
                velocity: 100,
            },
        }))
    }
}

fn sort_sample_entries(entries: &mut [SampleEntry]) {
    entries.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            return b.is_dir.cmp(&a.is_dir);
        }
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });
}
