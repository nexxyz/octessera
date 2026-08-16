use crate::native_menu::{sample_display_name, NativeMenuAction, NativeSampleAvailability};
use crate::protocol::{RuntimeAudioCommand, RuntimePlatformEffect, SampleEntry};

use super::{parent_dir, parse_sample_action, same_sample_path, NativeRunner, NativeSampleBrowser};

impl NativeRunner {
    pub(super) fn handle_sample_browser_action(
        &mut self,
        action: &str,
    ) -> Result<Option<RuntimePlatformEffect>, String> {
        if let Some(rest) = action.strip_prefix("sample.open:") {
            let (instrument_slot, sample_slot, dir) = parse_sample_action(rest)?;
            let dir = dir.unwrap_or_default();
            self.sample_browser = Some(NativeSampleBrowser {
                instrument_slot,
                sample_slot,
                dir: dir.clone(),
                entries: vec![],
            });
            self.menu.rebuild(self.menu_config());
            return Ok(Some(RuntimePlatformEffect::SampleListRequest {
                instrument_slot,
                sample_slot,
                dir,
            }));
        }
        if let Some(rest) = action.strip_prefix("sample.enter:") {
            let (instrument_slot, sample_slot, dir) = parse_sample_action(rest)?;
            let dir = dir.unwrap_or_default();
            self.sample_browser = Some(NativeSampleBrowser {
                instrument_slot,
                sample_slot,
                dir: dir.clone(),
                entries: vec![],
            });
            self.menu.rebuild(self.menu_config());
            return Ok(Some(RuntimePlatformEffect::SampleListRequest {
                instrument_slot,
                sample_slot,
                dir,
            }));
        }
        if let Some(rest) = action.strip_prefix("sample.up:") {
            let (instrument_slot, sample_slot, _) = parse_sample_action(rest)?;
            let dir = self
                .sample_browser
                .as_ref()
                .map(|browser| parent_dir(&browser.dir))
                .unwrap_or_default();
            self.sample_browser = Some(NativeSampleBrowser {
                instrument_slot,
                sample_slot,
                dir: dir.clone(),
                entries: vec![],
            });
            self.menu.rebuild(self.menu_config());
            return Ok(Some(RuntimePlatformEffect::SampleListRequest {
                instrument_slot,
                sample_slot,
                dir,
            }));
        }
        if let Some(rest) = action.strip_prefix("sample.pick:") {
            let (instrument_slot, sample_slot, path) = parse_sample_action(rest)?;
            let Some(path) = path else {
                return Ok(None);
            };
            let keep_unavailable = self
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
                    if !keep_unavailable {
                        if let Some(availability) = self
                            .sample_availability
                            .get_mut(instrument_slot)
                            .and_then(|slots| slots.get_mut(sample_slot))
                        {
                            *availability = NativeSampleAvailability::Available;
                        }
                    }
                    changed = true;
                }
            }
            if changed {
                if let Some(config) = self.instrument_audio_config(instrument_slot) {
                    self.queue_audio_command(RuntimeAudioCommand::SetInstrumentSlot {
                        instrument_slot,
                        config,
                    });
                }
                self.mark_fast_autosave_dirty();
                self.sample_browser = None;
                self.menu.rebuild(self.menu_config());
                let _ = self
                    .menu
                    .focus_item_key(&format!("sample.choose:{instrument_slot}:{sample_slot}"));
            }
            return Ok(None);
        }
        if let Some(rest) = action.strip_prefix("sample.favorite.set:") {
            let (instrument_slot, sample_slot, _) = parse_sample_action(rest)?;
            return self.toggle_sample_favourite(instrument_slot, sample_slot, true);
        }
        if let Some(rest) = action.strip_prefix("sample.favorite.remove:") {
            let (instrument_slot, sample_slot, _) = parse_sample_action(rest)?;
            return self.toggle_sample_favourite(instrument_slot, sample_slot, false);
        }
        if let Some(rest) = action.strip_prefix("sample.preview:") {
            let (instrument_slot, sample_slot, path) = parse_sample_action(rest)?;
            if let Some(path) = path {
                return Ok(Some(RuntimePlatformEffect::AudioCommand {
                    command: RuntimeAudioCommand::SamplePreview {
                        instrument_slot,
                        sample_slot,
                        path,
                        velocity: 100,
                    },
                }));
            }
            return Ok(None);
        }
        Ok(None)
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

    fn mark_sample_available(&mut self, instrument_slot: usize, sample_slot: usize) {
        let Some(availability) = self
            .sample_availability
            .get_mut(instrument_slot)
            .and_then(|slots| slots.get_mut(sample_slot))
        else {
            return;
        };
        if *availability != NativeSampleAvailability::Unavailable {
            *availability = NativeSampleAvailability::Available;
        }
    }

    fn mark_sample_unavailable(&mut self, instrument_slot: usize, sample_slot: usize) {
        if let Some(availability) = self
            .sample_availability
            .get_mut(instrument_slot)
            .and_then(|slots| slots.get_mut(sample_slot))
        {
            *availability = NativeSampleAvailability::Unavailable;
        }
    }

    pub(super) fn mark_sample_unavailable_from_error(
        &mut self,
        error: &crate::protocol::RuntimeErrorFacts,
    ) -> bool {
        if error.domain != crate::RuntimeErrorDomain::Sample {
            return false;
        }
        let Some(message) = error.message.as_deref() else {
            return false;
        };
        let mut changed = false;
        let matching_slots =
            self.instruments
                .iter()
                .enumerate()
                .flat_map(|(instrument_slot, instrument)| {
                    instrument.sample_paths.iter().enumerate().filter_map(
                        move |(sample_slot, path)| {
                            path.as_deref()
                                .filter(|path| message.contains(path))
                                .map(|_| (instrument_slot, sample_slot))
                        },
                    )
                })
                .collect::<Vec<_>>();
        for (instrument_slot, sample_slot) in matching_slots {
            let unavailable = self
                .sample_availability
                .get(instrument_slot)
                .and_then(|slots| slots.get(sample_slot))
                == Some(&NativeSampleAvailability::Unavailable);
            if !unavailable {
                self.mark_sample_unavailable(instrument_slot, sample_slot);
                changed = true;
            }
        }
        changed
    }

    pub(super) fn preserve_sample_availability_from(&mut self, source: &NativeRunner) {
        for (instrument_slot, instrument) in self.instruments.iter().enumerate() {
            for (sample_slot, path) in instrument.sample_paths.iter().enumerate() {
                let same_path = path
                    .as_deref()
                    .zip(
                        source
                            .instruments
                            .get(instrument_slot)
                            .and_then(|instrument| instrument.sample_paths.get(sample_slot))
                            .and_then(Option::as_deref),
                    )
                    .is_some_and(|(left, right)| same_sample_path(left, right));
                if !same_path {
                    if let Some(availability) = self
                        .sample_availability
                        .get_mut(instrument_slot)
                        .and_then(|slots| slots.get_mut(sample_slot))
                    {
                        *availability = NativeSampleAvailability::Unknown;
                    }
                    continue;
                }
                if let Some(availability) = self
                    .sample_availability
                    .get_mut(instrument_slot)
                    .and_then(|slots| slots.get_mut(sample_slot))
                {
                    *availability = source
                        .sample_availability
                        .get(instrument_slot)
                        .and_then(|slots| slots.get(sample_slot))
                        .copied()
                        .unwrap_or(NativeSampleAvailability::Unknown);
                }
            }
        }
    }

    fn toggle_sample_favourite(
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
                self.mark_config_dirty();
            }
            self.show_toast("Favourite set");
        } else if let Some(index) = self
            .sample_favourite_dirs
            .iter()
            .position(|entry| entry == &dir)
        {
            self.sample_favourite_dirs.remove(index);
            self.mark_config_dirty();
            self.show_toast("Favourite removed");
        } else {
            return Ok(None);
        }
        self.menu.rebuild(self.menu_config());
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
        let (instrument_slot, sample_slot, _) = parse_sample_action(key).ok()?;
        let dir = self
            .sample_browser
            .as_ref()
            .filter(|browser| {
                browser.instrument_slot == instrument_slot && browser.sample_slot == sample_slot
            })
            .map(|browser| browser.dir.clone())
            .unwrap_or_default();
        self.sample_browser = Some(NativeSampleBrowser {
            instrument_slot,
            sample_slot,
            dir: dir.clone(),
            entries: vec![],
        });
        self.menu.rebuild(self.menu_config());
        Some(RuntimePlatformEffect::SampleListRequest {
            instrument_slot,
            sample_slot,
            dir,
        })
    }

    pub(super) fn preview_selected_sample(&self) -> Result<Option<RuntimePlatformEffect>, String> {
        let Some(NativeMenuAction::PlatformEffect(action)) = self.menu.snapshot().selected_action
        else {
            return Ok(None);
        };
        let Some(rest) = action.strip_prefix("sample.pick:") else {
            return Ok(None);
        };
        let (instrument_slot, sample_slot, path) = parse_sample_action(rest)?;
        let Some(path) = path else {
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
