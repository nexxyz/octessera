use crate::native_menu::NativeMenuAction;
use crate::protocol::{RuntimeAudioCommand, RuntimePlatformEffect};

use super::{parent_dir, parse_sample_action, NativeRunner, NativeSampleBrowser};

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
            let mut changed = false;
            if let Some(instrument) = self.instruments.get_mut(instrument_slot) {
                if sample_slot < instrument.sample_paths.len() {
                    instrument.sample_paths[sample_slot] = Some(path);
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
                self.config_dirty = true;
            }
            self.show_toast("Favourite set");
        } else if let Some(index) = self
            .sample_favourite_dirs
            .iter()
            .position(|entry| entry == &dir)
        {
            self.sample_favourite_dirs.remove(index);
            self.config_dirty = true;
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
