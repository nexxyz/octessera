use crate::events::MusicalEvent;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteBehavior {
    Hold,
    Oneshot,
}

impl NoteBehavior {
    pub const ALL: [Self; 2] = [Self::Oneshot, Self::Hold];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Hold => "hold",
            Self::Oneshot => "oneshot",
        }
    }

    pub fn from_wire_name(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|behavior| behavior.wire_name() == value)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
