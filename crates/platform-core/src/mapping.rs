use crate::events::MusicalEvent;
use crate::interpretation::{CellTriggerIntent, CellTriggerKind};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerAction {
    None,
    NoteOn,
    NoteOff,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerTarget {
    pub action: TriggerAction,
    pub channel: u8,
    pub velocity: u8,
    #[serde(rename = "durationMs")]
    pub duration_ms: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RangeMode {
    Clamp,
    Wrap,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MappingConfig {
    #[serde(rename = "baseMidiNote")]
    pub base_midi_note: i32,
    #[serde(rename = "startingMidiNote")]
    pub starting_midi_note: i32,
    #[serde(rename = "maxMidiNote")]
    pub max_midi_note: i32,
    #[serde(rename = "rangeMode")]
    pub range_mode: RangeMode,
    pub scale: Vec<i32>,
    #[serde(rename = "rowStepDegrees")]
    pub row_step_degrees: i32,
    #[serde(rename = "columnStepDegrees")]
    pub column_step_degrees: i32,
    pub activate: TriggerTarget,
    pub deactivate: TriggerTarget,
    pub stable: TriggerTarget,
    pub scanned: TriggerTarget,
    pub scanned_empty: TriggerTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MappingResult {
    pub events: Vec<MusicalEvent>,
    pub intents: Vec<CellTriggerIntent>,
}

pub fn default_mapping_config() -> MappingConfig {
    MappingConfig {
        base_midi_note: 24,
        starting_midi_note: 24,
        max_midi_note: 84,
        range_mode: RangeMode::Wrap,
        scale: vec![0, 3, 5, 7, 10],
        row_step_degrees: 3,
        column_step_degrees: 1,
        activate: TriggerTarget {
            action: TriggerAction::NoteOn,
            channel: 0,
            velocity: 96,
            duration_ms: 150,
        },
        deactivate: TriggerTarget {
            action: TriggerAction::NoteOff,
            channel: 0,
            velocity: 68,
            duration_ms: 90,
        },
        stable: TriggerTarget {
            action: TriggerAction::None,
            channel: 0,
            velocity: 88,
            duration_ms: 130,
        },
        scanned: TriggerTarget {
            action: TriggerAction::NoteOn,
            channel: 0,
            velocity: 88,
            duration_ms: 130,
        },
        scanned_empty: TriggerTarget {
            action: TriggerAction::None,
            channel: 0,
            velocity: 68,
            duration_ms: 90,
        },
    }
}

pub fn map_intents_to_musical_events(
    intents: &[CellTriggerIntent],
    config: &MappingConfig,
) -> MappingResult {
    let safe = validate_config(config);
    let mut events = Vec::new();
    let mut matched = Vec::new();
    for intent in intents {
        let note = note_from_degree(intent.degree, &safe);
        let target = target_for_kind(intent.kind, &safe);
        match target.action {
            TriggerAction::None => continue,
            TriggerAction::NoteOff => {
                events.push(MusicalEvent::NoteOff {
                    channel: target.channel,
                    note: note as u8,
                });
            }
            TriggerAction::NoteOn => {
                events.push(MusicalEvent::NoteOn {
                    channel: target.channel,
                    note: note as u8,
                    velocity: target.velocity,
                    duration_ms: Some(target.duration_ms),
                });
                matched.push(intent.clone());
            }
        }
    }
    MappingResult {
        events,
        intents: matched,
    }
}

fn target_for_kind(kind: CellTriggerKind, config: &MappingConfig) -> &TriggerTarget {
    match kind {
        CellTriggerKind::Activate => &config.activate,
        CellTriggerKind::Deactivate => &config.deactivate,
        CellTriggerKind::ScannedEmpty => &config.scanned_empty,
        CellTriggerKind::Scanned => &config.scanned,
        CellTriggerKind::Stable => &config.stable,
    }
}

fn note_from_degree(degree: i32, config: &MappingConfig) -> i32 {
    let scale_notes = scale_notes_in_range(config);
    if scale_notes.is_empty() {
        return clamp(
            config.base_midi_note,
            config.base_midi_note,
            config.max_midi_note,
        );
    }
    let start_index = nearest_scale_index(&scale_notes, config.starting_midi_note);
    let target_index = start_index as i32 + degree;
    match config.range_mode {
        RangeMode::Wrap => scale_notes[modulo(target_index, scale_notes.len() as i32) as usize],
        RangeMode::Clamp => {
            scale_notes[clamp(target_index, 0, scale_notes.len() as i32 - 1) as usize]
        }
    }
}

fn validate_config(config: &MappingConfig) -> MappingConfig {
    let scale = if config.scale.is_empty() {
        vec![0]
    } else {
        config.scale.clone()
    };
    let base_midi_note = clamp(config.base_midi_note, 0, 127);
    let starting_midi_note = clamp(config.starting_midi_note, base_midi_note, 127);
    let max_midi_note = clamp(config.max_midi_note, base_midi_note, 127);
    MappingConfig {
        base_midi_note,
        starting_midi_note: clamp(starting_midi_note, base_midi_note, max_midi_note),
        max_midi_note,
        range_mode: config.range_mode,
        scale: scale.into_iter().map(|step| clamp(step, 0, 11)).collect(),
        row_step_degrees: config.row_step_degrees.max(0),
        column_step_degrees: config.column_step_degrees.max(0),
        activate: sanitize_target(&config.activate),
        deactivate: sanitize_target(&config.deactivate),
        stable: sanitize_target(&config.stable),
        scanned: sanitize_target(&config.scanned),
        scanned_empty: sanitize_target(&config.scanned_empty),
    }
}

fn sanitize_target(target: &TriggerTarget) -> TriggerTarget {
    TriggerTarget {
        action: target.action,
        channel: target.channel.min(15),
        velocity: target.velocity.clamp(1, 127),
        duration_ms: target.duration_ms.clamp(1, 8000),
    }
}

fn scale_notes_in_range(config: &MappingConfig) -> Vec<i32> {
    (config.base_midi_note..=config.max_midi_note)
        .filter(|note| {
            let pitch_class = note.rem_euclid(12);
            config.scale.contains(&pitch_class)
        })
        .collect()
}

fn nearest_scale_index(scale_notes: &[i32], note: i32) -> usize {
    scale_notes
        .iter()
        .enumerate()
        .min_by_key(|(_, candidate)| {
            let distance = (*candidate - note).abs();
            (distance, **candidate)
        })
        .map(|(index, _)| index)
        .unwrap_or(0)
}

#[cfg(test)]
fn note_matches_scale(note: u8, config: &MappingConfig) -> bool {
    let pitch_class = i32::from(note % 12);
    config.scale.contains(&pitch_class)
}

fn clamp(value: i32, min: i32, max: i32) -> i32 {
    value.max(min).min(max)
}

fn modulo(value: i32, base: i32) -> i32 {
    value.rem_euclid(base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpretation::{CellTriggerIntent, CellTriggerKind};

    #[test]
    fn maps_note_on_and_note_off() {
        let config = default_mapping_config();
        let result = map_intents_to_musical_events(
            &[
                CellTriggerIntent {
                    x: 0,
                    y: 0,
                    kind: CellTriggerKind::Activate,
                    degree: 0,
                },
                CellTriggerIntent {
                    x: 1,
                    y: 0,
                    kind: CellTriggerKind::Deactivate,
                    degree: 1,
                },
                CellTriggerIntent {
                    x: 2,
                    y: 0,
                    kind: CellTriggerKind::Stable,
                    degree: 2,
                },
            ],
            &config,
        );

        assert_eq!(
            result.events,
            vec![
                MusicalEvent::NoteOn {
                    channel: 0,
                    note: 24,
                    velocity: 96,
                    duration_ms: Some(150)
                },
                MusicalEvent::NoteOff {
                    channel: 0,
                    note: 27
                },
            ]
        );
        assert_eq!(result.intents.len(), 1);
    }

    #[test]
    fn wraps_notes_into_range() {
        let mut config = default_mapping_config();
        config.base_midi_note = 60;
        config.max_midi_note = 64;
        let result = map_intents_to_musical_events(
            &[CellTriggerIntent {
                x: 0,
                y: 0,
                kind: CellTriggerKind::Activate,
                degree: 9,
            }],
            &config,
        );
        assert_eq!(
            result.events,
            vec![MusicalEvent::NoteOn {
                channel: 0,
                note: 63,
                velocity: 96,
                duration_ms: Some(150)
            }]
        );
    }

    #[test]
    fn wrapped_notes_stay_on_scale_when_starting_note_is_off_scale() {
        let mut config = default_mapping_config();
        config.base_midi_note = 61;
        config.max_midi_note = 72;
        config.scale = vec![0, 2, 4, 5, 7, 9, 11];
        config.range_mode = RangeMode::Wrap;

        let result = map_intents_to_musical_events(
            &[CellTriggerIntent {
                x: 0,
                y: 0,
                kind: CellTriggerKind::Activate,
                degree: 64,
            }],
            &config,
        );

        let MusicalEvent::NoteOn { note, .. } = result.events[0] else {
            panic!("expected note on");
        };
        assert!(note_matches_scale(note, &validate_config(&config)));
        assert_ne!(note, 61);
    }

    #[test]
    fn negative_wrapped_notes_stay_on_selected_scale() {
        let mut config = default_mapping_config();
        config.base_midi_note = 60;
        config.starting_midi_note = 64;
        config.max_midi_note = 72;
        config.scale = vec![0, 2, 4, 7, 9];
        config.range_mode = RangeMode::Wrap;

        let result = map_intents_to_musical_events(
            &[CellTriggerIntent {
                x: 0,
                y: 0,
                kind: CellTriggerKind::Activate,
                degree: -3,
            }],
            &config,
        );

        let MusicalEvent::NoteOn { note, .. } = result.events[0] else {
            panic!("expected note on");
        };
        assert!(note_matches_scale(note, &validate_config(&config)));
    }

    #[test]
    fn supports_none_note_off_and_scanned_empty_targets() {
        let mut config = default_mapping_config();
        config.scanned_empty = TriggerTarget {
            action: TriggerAction::NoteOn,
            channel: 3,
            velocity: 55,
            duration_ms: 70,
        };
        config.scanned.action = TriggerAction::None;
        let result = map_intents_to_musical_events(
            &[
                CellTriggerIntent {
                    x: 0,
                    y: 0,
                    kind: CellTriggerKind::Scanned,
                    degree: 0,
                },
                CellTriggerIntent {
                    x: 1,
                    y: 0,
                    kind: CellTriggerKind::ScannedEmpty,
                    degree: 1,
                },
                CellTriggerIntent {
                    x: 2,
                    y: 0,
                    kind: CellTriggerKind::Deactivate,
                    degree: 2,
                },
            ],
            &config,
        );
        assert_eq!(result.events.len(), 2);
        assert_eq!(
            result.events[0],
            MusicalEvent::NoteOn {
                channel: 3,
                note: 27,
                velocity: 55,
                duration_ms: Some(70)
            }
        );
        assert!(matches!(
            result.events[1],
            MusicalEvent::NoteOff { note: 29, .. }
        ));
    }

    #[test]
    fn mapping_config_sanitizes_out_of_range_values_and_empty_scale() {
        let mut config = default_mapping_config();
        config.base_midi_note = -10;
        config.max_midi_note = 200;
        config.scale = vec![];
        config.row_step_degrees = -5;
        config.column_step_degrees = -2;
        config.activate.channel = 99;
        config.activate.velocity = 250;
        config.activate.duration_ms = 0;

        let result = map_intents_to_musical_events(
            &[CellTriggerIntent {
                x: 0,
                y: 0,
                kind: CellTriggerKind::Activate,
                degree: 0,
            }],
            &config,
        );

        assert_eq!(
            result.events,
            vec![MusicalEvent::NoteOn {
                channel: 15,
                note: 24,
                velocity: 127,
                duration_ms: Some(1)
            }]
        );
    }

    #[test]
    fn range_mode_clamp_and_wrap_differ_for_high_degree() {
        let mut config = default_mapping_config();
        config.base_midi_note = 60;
        config.max_midi_note = 64;
        config.scale = vec![0, 2, 4, 5, 7];
        config.range_mode = RangeMode::Clamp;
        let clamped = map_intents_to_musical_events(
            &[CellTriggerIntent {
                x: 0,
                y: 0,
                kind: CellTriggerKind::Activate,
                degree: 9,
            }],
            &config,
        );
        config.range_mode = RangeMode::Wrap;
        let wrapped = map_intents_to_musical_events(
            &[CellTriggerIntent {
                x: 0,
                y: 0,
                kind: CellTriggerKind::Activate,
                degree: 9,
            }],
            &config,
        );

        assert_ne!(clamped.events, wrapped.events);
        assert_eq!(
            clamped.events[0],
            MusicalEvent::NoteOn {
                channel: 0,
                note: 64,
                velocity: 96,
                duration_ms: Some(150)
            }
        );
    }
}
