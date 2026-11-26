use super::sample_paths::same_sample_path;
use super::{NativeHelpPopup, NativeRunner, NativeSampleAvailability};
use crate::oled_frame::{wrap_text, MENU_BODY_RECT};

const MISSING_SAMPLES_TITLE: &str = "Samples missing";
const MISSING_SAMPLES_INTRO: &str =
    "Missing, damaged or too long, so these slots stay silent. Pick a replacement sample:";
const SD_CARD_REPAIRED_TITLE: &str = "SD card repaired";
const SD_CARD_REPAIRED_TEXT: &str =
    "It was pulled or cut off mid-write, so a quick check tidied it up. A file written right then may be missing.";

impl NativeRunner {
    pub(super) fn apply_missing_samples(
        &mut self,
        only_instrument: Option<usize>,
        paths: &[String],
    ) {
        let newly_missing =
            self.instruments
                .iter()
                .enumerate()
                .filter(|(instrument_slot, _)| {
                    only_instrument.is_none_or(|only| only == *instrument_slot)
                })
                .flat_map(|(instrument_slot, instrument)| {
                    instrument.sample_paths.iter().enumerate().filter_map(
                        move |(sample_slot, path)| {
                            let path = path.as_deref()?;
                            paths
                                .iter()
                                .any(|missing| same_sample_path(missing, path))
                                .then(|| (instrument_slot, sample_slot, path.to_string()))
                        },
                    )
                })
                .filter(|(instrument_slot, sample_slot, _)| {
                    !self.sample_unavailable(*instrument_slot, *sample_slot)
                })
                .collect::<Vec<_>>();
        if newly_missing.is_empty() {
            return;
        }
        let columns = MENU_BODY_RECT.columns();
        let mut lines = wrap_text(MISSING_SAMPLES_INTRO, columns);
        let mut listed = Vec::<&str>::new();
        let mut changed_instruments = Vec::new();
        for (instrument_slot, sample_slot, path) in &newly_missing {
            self.mark_sample_unavailable(*instrument_slot, *sample_slot);
            if !changed_instruments.contains(instrument_slot) {
                changed_instruments.push(*instrument_slot);
            }
            if !listed.iter().any(|seen| same_sample_path(seen, path)) {
                listed.push(path);
                lines.extend(wrap_text(
                    &format!("I{} {path}", instrument_slot + 1),
                    columns,
                ));
            }
        }
        self.display.help_popup = Some(NativeHelpPopup {
            title: MISSING_SAMPLES_TITLE.into(),
            lines,
            scroll: 0,
        });
        self.show_instruments_rows(&changed_instruments);
    }

    /// The missing-samples popup outranks this one, since it names slots to fix.
    pub(super) fn show_sd_card_repaired(&mut self) {
        if self
            .display
            .help_popup
            .as_ref()
            .is_some_and(|popup| popup.title == MISSING_SAMPLES_TITLE)
        {
            return;
        }
        self.display.help_popup = Some(NativeHelpPopup {
            title: SD_CARD_REPAIRED_TITLE.into(),
            lines: wrap_text(SD_CARD_REPAIRED_TEXT, MENU_BODY_RECT.columns()),
            scroll: 0,
        });
    }

    /// A card arrived in SD2: instruments with a card sample that has not loaded load again
    /// through their normal audio load. Slots already playing keep their decoded sample.
    pub(super) fn reload_sd_card_samples(&mut self) {
        let instruments = self
            .instruments
            .iter()
            .enumerate()
            .filter(|(instrument_slot, instrument)| {
                instrument
                    .sample_paths
                    .iter()
                    .enumerate()
                    .any(|(sample_slot, path)| {
                        path.as_deref().is_some_and(is_sd_card_sample_path)
                            && self.sample_unavailable(*instrument_slot, sample_slot)
                    })
            })
            .map(|(instrument_slot, _)| instrument_slot)
            .collect::<Vec<_>>();
        for instrument_slot in instruments {
            self.queue_instrument_slot_config(instrument_slot);
        }
    }

    pub(super) fn apply_loaded_samples(&mut self, only_instrument: usize, paths: &[String]) {
        let loaded =
            self.instruments
                .iter()
                .enumerate()
                .filter(|(instrument_slot, _)| *instrument_slot == only_instrument)
                .flat_map(|(instrument_slot, instrument)| {
                    instrument.sample_paths.iter().enumerate().filter_map(
                        move |(sample_slot, path)| {
                            let path = path.as_deref()?;
                            paths
                                .iter()
                                .any(|loaded| same_sample_path(loaded, path))
                                .then_some((instrument_slot, sample_slot))
                        },
                    )
                })
                .filter(|(instrument_slot, sample_slot)| {
                    self.sample_unavailable(*instrument_slot, *sample_slot)
                })
                .collect::<Vec<_>>();
        if loaded.is_empty() {
            return;
        }
        let mut changed_instruments = Vec::new();
        for (instrument_slot, sample_slot) in loaded {
            self.mark_sample_available(instrument_slot, sample_slot);
            if !changed_instruments.contains(&instrument_slot) {
                changed_instruments.push(instrument_slot);
            }
        }
        let nothing_missing = self
            .sample_availability
            .iter()
            .flatten()
            .all(|availability| *availability != NativeSampleAvailability::Unavailable);
        if nothing_missing
            && self
                .display
                .help_popup
                .as_ref()
                .is_some_and(|popup| popup.title == MISSING_SAMPLES_TITLE)
        {
            self.display.help_popup = None;
        }
        self.show_instruments_rows(&changed_instruments);
    }

    fn sample_unavailable(&self, instrument_slot: usize, sample_slot: usize) -> bool {
        self.sample_availability
            .get(instrument_slot)
            .and_then(|slots| slots.get(sample_slot))
            == Some(&NativeSampleAvailability::Unavailable)
    }
}

fn is_sd_card_sample_path(path: &str) -> bool {
    let path = path.replace('\\', "/");
    path == "sd-card" || path.starts_with("sd-card/")
}

#[cfg(test)]
mod tests;
