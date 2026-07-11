use crate::events::MusicalEvent;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteBehavior {
    Hold,
    Oneshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VelocityCurve {
    Linear,
    Soft,
    Hard,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GlobalSoundConfig {
    #[serde(rename = "velocityScalePct")]
    pub velocity_scale_pct: u16,
    #[serde(rename = "velocityCurve")]
    pub velocity_curve: VelocityCurve,
    #[serde(rename = "noteLengthMs")]
    pub note_length_ms: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteBehaviorResult {
    pub events: Vec<MusicalEvent>,
    pub held_notes: Vec<String>,
}

pub fn dedupe_simultaneous_notes(events: &[MusicalEvent]) -> Vec<MusicalEvent> {
    let mut out = Vec::with_capacity(events.len());
    let mut seen = HashMap::<(u8, u8), usize>::with_capacity(events.len());
    for event in events {
        match event {
            MusicalEvent::NoteOn {
                channel,
                note,
                velocity,
                duration_ms,
            } => {
                let key = (*channel, *note);
                if let Some(index) = seen.get(&key).copied() {
                    if let MusicalEvent::NoteOn {
                        velocity: existing_velocity,
                        duration_ms: existing_duration,
                        ..
                    } = &mut out[index]
                    {
                        *existing_velocity = (*existing_velocity).max(*velocity);
                        *existing_duration =
                            Some(existing_duration.unwrap_or(0).max(duration_ms.unwrap_or(0)));
                    }
                } else {
                    seen.insert(key, out.len());
                    out.push(event.clone());
                }
            }
            _ => out.push(event.clone()),
        }
    }
    out
}

pub fn apply_global_sound(
    events: &[MusicalEvent],
    config: &GlobalSoundConfig,
) -> Vec<MusicalEvent> {
    let scale = (config.velocity_scale_pct as f32 / 100.0).clamp(0.0, 2.0);
    let note_length_ms = config.note_length_ms.clamp(1, 10_000);
    events
        .iter()
        .map(|event| match event {
            MusicalEvent::NoteOn {
                channel,
                note,
                velocity,
                duration_ms,
            } => {
                let normalized = (*velocity).clamp(1, 127) as f32 / 127.0;
                let shaped = match config.velocity_curve {
                    VelocityCurve::Linear => normalized,
                    VelocityCurve::Soft => normalized.sqrt(),
                    VelocityCurve::Hard => normalized * normalized,
                };
                let next_velocity = (shaped * 127.0 * scale).round().clamp(1.0, 127.0) as u8;
                MusicalEvent::NoteOn {
                    channel: *channel,
                    note: *note,
                    velocity: next_velocity,
                    duration_ms: Some(duration_ms.unwrap_or(note_length_ms)),
                }
            }
            _ => event.clone(),
        })
        .collect()
}

pub fn apply_note_behavior(
    events: &[MusicalEvent],
    behaviors: &[NoteBehavior],
    layer_idx: usize,
    initial_held: &[String],
) -> NoteBehaviorResult {
    let mut held = initial_held.iter().cloned().collect::<HashSet<_>>();
    held.reserve(events.len());
    let mut out = Vec::with_capacity(events.len());
    for event in events {
        match event {
            MusicalEvent::NoteOn {
                channel,
                note,
                velocity,
                duration_ms,
            } => {
                let key = format!("{layer_idx}:{channel}:{note}");
                let behavior = behaviors
                    .get(*channel as usize)
                    .copied()
                    .unwrap_or(NoteBehavior::Oneshot);
                if behavior == NoteBehavior::Hold && held.contains(&key) {
                    continue;
                }
                if behavior == NoteBehavior::Hold {
                    held.insert(key);
                    out.push(MusicalEvent::NoteOn {
                        channel: *channel,
                        note: *note,
                        velocity: *velocity,
                        duration_ms: None,
                    });
                } else {
                    out.push(MusicalEvent::NoteOn {
                        channel: *channel,
                        note: *note,
                        velocity: *velocity,
                        duration_ms: *duration_ms,
                    });
                }
            }
            MusicalEvent::NoteOff { channel, note } => {
                let key = format!("{layer_idx}:{channel}:{note}");
                let _ = held.remove(&key);
                out.push(event.clone());
            }
            _ => out.push(event.clone()),
        }
    }
    let mut held_notes = held.into_iter().collect::<Vec<_>>();
    held_notes.sort();
    NoteBehaviorResult {
        events: out,
        held_notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedupes_note_ons_by_max_values() {
        let events = vec![
            MusicalEvent::NoteOn {
                channel: 1,
                note: 60,
                velocity: 90,
                duration_ms: Some(10),
            },
            MusicalEvent::Cc {
                channel: 1,
                controller: 74,
                value: 40,
            },
            MusicalEvent::NoteOn {
                channel: 1,
                note: 60,
                velocity: 100,
                duration_ms: Some(20),
            },
        ];
        assert_eq!(
            dedupe_simultaneous_notes(&events),
            vec![
                MusicalEvent::NoteOn {
                    channel: 1,
                    note: 60,
                    velocity: 100,
                    duration_ms: Some(20)
                },
                MusicalEvent::Cc {
                    channel: 1,
                    controller: 74,
                    value: 40
                },
            ]
        );
    }

    #[test]
    fn applies_global_sound_defaults_duration() {
        let events = vec![MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 64,
            duration_ms: None,
        }];
        let config = GlobalSoundConfig {
            velocity_scale_pct: 100,
            velocity_curve: VelocityCurve::Linear,
            note_length_ms: 180,
        };
        assert_eq!(
            apply_global_sound(&events, &config),
            vec![MusicalEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 64,
                duration_ms: Some(180),
            }]
        );
    }

    #[test]
    fn applies_hold_note_behavior() {
        let events = vec![
            MusicalEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
                duration_ms: Some(120),
            },
            MusicalEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
                duration_ms: Some(120),
            },
            MusicalEvent::NoteOff {
                channel: 0,
                note: 60,
            },
        ];
        let result = apply_note_behavior(&events, &[NoteBehavior::Hold], 2, &[]);
        assert_eq!(
            result.events,
            vec![
                MusicalEvent::NoteOn {
                    channel: 0,
                    note: 60,
                    velocity: 100,
                    duration_ms: None
                },
                MusicalEvent::NoteOff {
                    channel: 0,
                    note: 60
                },
            ]
        );
        assert!(result.held_notes.is_empty());
    }
}
