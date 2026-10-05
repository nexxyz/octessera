use super::super::model::{CheckOutcome, CheckStatus};
use super::readiness;
use super::support::{outcome, outcome_with_content, read_small};
use super::CheckContext;
use std::path::Path;

const ORANGE_ASOUND_CARDS: &str = "/proc/asound/cards";
const ORANGE_ASOUND_PCM: &str = "/proc/asound/pcm";
const ORANGE_CARD_ID: &str = "octesseradac";
const ORANGE_CARD_DESCRIPTION: &str = "octessera-dac - octessera-dac";

pub(super) fn oled_handoff_check(context: &CheckContext) -> CheckOutcome {
    orange_oled_handoff_from_readiness(readiness::readiness_check(context))
}

pub(super) fn orange_oled_handoff_from_readiness(readiness: CheckOutcome) -> CheckOutcome {
    let status = readiness.status;
    let artifact_content = readiness.artifact_content;
    let message = if status == CheckStatus::Pass {
        "Orange OLED/native handoff is covered by the current readiness marker and matching service invocation"
    } else {
        "current readiness marker and matching service invocation do not establish Orange OLED/native handoff"
    };
    outcome_with_content(status, message, "06-oled-handoff.txt", &artifact_content)
}

pub(super) fn audio_route_check(context: &CheckContext) -> CheckOutcome {
    let cards = match read_small(Path::new(ORANGE_ASOUND_CARDS)) {
        Ok(content) => content,
        Err(error) => return outcome(CheckStatus::Fail, &error, "07-audio.txt"),
    };
    let pcm = match read_small(Path::new(ORANGE_ASOUND_PCM)) {
        Ok(content) => content,
        Err(error) => return outcome(CheckStatus::Fail, &error, "07-audio.txt"),
    };
    let artifact = format!("cards={cards}\npcm={pcm}");
    if !orange_audio_route_matches(&cards, &pcm) {
        return outcome_with_content(
            CheckStatus::Fail,
            "fixed Orange audio card and playback PCM are not listed",
            "07-audio.txt",
            &artifact,
        );
    }
    outcome_with_content(
        CheckStatus::Pass,
        &format!(
            "fixed Orange audio route is listed; selected route={}",
            context.board.audio_route
        ),
        "07-audio.txt",
        &artifact,
    )
}

pub(super) fn orange_audio_route_matches(cards: &str, pcm: &str) -> bool {
    let Some(card_index) = orange_card_index(cards) else {
        return false;
    };
    orange_playback_pcm_is_present(pcm, card_index)
}

fn orange_card_index(cards: &str) -> Option<u32> {
    let mut matching_entries = 0;
    let mut card_index = None;
    for line in cards.lines() {
        let Some((number, remainder)) = line.split_once('[') else {
            continue;
        };
        let Some((identifier, description)) = remainder.split_once(']') else {
            continue;
        };
        if identifier.trim() != ORANGE_CARD_ID {
            continue;
        }
        matching_entries += 1;
        let Ok(number) = number.trim().parse::<u32>() else {
            return None;
        };
        let description = description.trim().strip_prefix(':')?;
        if description.trim() != ORANGE_CARD_DESCRIPTION {
            return None;
        }
        if card_index.replace(number).is_some() {
            return None;
        }
    }
    (matching_entries == 1).then_some(card_index?)
}

fn orange_playback_pcm_is_present(pcm: &str, card_index: u32) -> bool {
    let expected_address = format!("{card_index:02}-00");
    let mut matching_entries = 0;
    for line in pcm.lines() {
        let Some((address, details)) = line.split_once(':') else {
            continue;
        };
        if address.trim() != expected_address {
            continue;
        }
        matching_entries += 1;
        let playback_streams = details
            .split(':')
            .filter(|section| section.trim() == "playback 1")
            .count();
        if playback_streams != 1 {
            return false;
        }
    }
    matching_entries == 1
}
