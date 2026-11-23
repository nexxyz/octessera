use super::sample_paths::same_sample_path;
use super::{NativeHelpPopup, NativeRunner, NativeSampleAvailability};
use crate::oled_frame::{wrap_text, MENU_BODY_RECT};

const MISSING_SAMPLES_TITLE: &str = "Samples missing";
const MISSING_SAMPLES_INTRO: &str =
    "Missing, damaged or too long, so these slots stay silent. Pick a replacement sample:";

impl NativeRunner {
    pub(super) fn apply_missing_samples(&mut self, paths: &[String]) {
        let newly_missing =
            self.instruments
                .iter()
                .enumerate()
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
                    self.sample_availability
                        .get(*instrument_slot)
                        .and_then(|slots| slots.get(*sample_slot))
                        != Some(&NativeSampleAvailability::Unavailable)
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

    /// SD2 came, went or was swapped: every slot with a sample on it is resolved again through
    /// the instrument's normal audio load, and the host reports the ones still missing.
    pub(super) fn reload_sd_card_samples(&mut self) {
        let sd_card_slots = self
            .instruments
            .iter()
            .enumerate()
            .flat_map(|(instrument_slot, instrument)| {
                instrument
                    .sample_paths
                    .iter()
                    .enumerate()
                    .filter(|(_, path)| path.as_deref().is_some_and(is_sd_card_sample_path))
                    .map(move |(sample_slot, _)| (instrument_slot, sample_slot))
            })
            .collect::<Vec<_>>();
        let mut reloaded_instruments = Vec::new();
        for (instrument_slot, sample_slot) in sd_card_slots {
            if let Some(availability) = self
                .sample_availability
                .get_mut(instrument_slot)
                .and_then(|slots| slots.get_mut(sample_slot))
            {
                *availability = NativeSampleAvailability::Unknown;
            }
            if !reloaded_instruments.contains(&instrument_slot) {
                reloaded_instruments.push(instrument_slot);
            }
        }
        if reloaded_instruments.is_empty() {
            return;
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
        for instrument_slot in &reloaded_instruments {
            self.queue_instrument_slot_config(*instrument_slot);
        }
        self.show_instruments_rows(&reloaded_instruments);
    }
}

fn is_sd_card_sample_path(path: &str) -> bool {
    let path = path.replace('\\', "/");
    path == "sd-card" || path.starts_with("sd-card/")
}

#[cfg(test)]
mod tests;
