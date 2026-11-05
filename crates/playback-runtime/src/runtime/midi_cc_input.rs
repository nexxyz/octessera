//! Incoming MIDI Control Change messages, kept as the latest value per channel and controller
//! until the host drains them into runner messages once per runtime tick.

use crate::protocol::HostMessage;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MidiControlChange {
    /// 0-15.
    pub channel: u8,
    pub controller: u8,
    pub value: u8,
}

impl MidiControlChange {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let [status, controller, value] = *bytes else {
            return None;
        };
        (status & 0xF0 == 0xB0 && controller < 0x80 && value < 0x80).then_some(Self {
            channel: status & 0x0F,
            controller,
            value,
        })
    }
}

/// A knob sweep between two ticks becomes one message per channel and controller.
#[derive(Debug, Default)]
pub struct MidiControlChangeBatch(Vec<MidiControlChange>);

impl MidiControlChangeBatch {
    pub fn push(&mut self, change: MidiControlChange) {
        match self
            .0
            .iter_mut()
            .find(|held| held.channel == change.channel && held.controller == change.controller)
        {
            Some(held) => held.value = change.value,
            None => self.0.push(change),
        }
    }
}

impl IntoIterator for MidiControlChangeBatch {
    type Item = MidiControlChange;
    type IntoIter = std::vec::IntoIter<MidiControlChange>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl From<MidiControlChange> for HostMessage {
    fn from(change: MidiControlChange) -> Self {
        Self::MidiControlChange {
            channel: change.channel,
            controller: change.controller,
            value: change.value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::support::{FakeHost, FakeRunner};
    use crate::{PlaybackRuntime, RuntimeConfig, RuntimeDispatchInput, SyncSource};

    #[test]
    fn only_three_byte_control_changes_parse() {
        assert_eq!(
            MidiControlChange::parse(&[0xB3, 74, 100]),
            Some(MidiControlChange {
                channel: 3,
                controller: 74,
                value: 100
            })
        );
        assert_eq!(MidiControlChange::parse(&[0x90, 60, 100]), None);
        assert_eq!(MidiControlChange::parse(&[0xB0, 74]), None);
        assert_eq!(MidiControlChange::parse(&[0xB0, 74, 0x80]), None);
        assert_eq!(MidiControlChange::parse(&[0xF8]), None);
    }

    #[test]
    fn a_batch_keeps_the_latest_value_per_channel_and_controller() {
        let mut batch = MidiControlChangeBatch::default();
        for (status, controller, value) in
            [(0xB0, 74, 1), (0xB0, 71, 2), (0xB0, 74, 3), (0xB1, 74, 4)]
        {
            batch.push(MidiControlChange::parse(&[status, controller, value]).unwrap());
        }
        let messages = batch.into_iter().map(HostMessage::from).collect::<Vec<_>>();
        assert_eq!(
            messages,
            vec![
                HostMessage::MidiControlChange {
                    channel: 0,
                    controller: 74,
                    value: 3
                },
                HostMessage::MidiControlChange {
                    channel: 0,
                    controller: 71,
                    value: 2
                },
                HostMessage::MidiControlChange {
                    channel: 1,
                    controller: 74,
                    value: 4
                },
            ]
        );
    }

    #[test]
    fn a_control_change_burst_follows_the_clock_pulses_of_its_drain_as_one_message() {
        let mut runtime = PlaybackRuntime::new(RuntimeConfig {
            bpm: 120.0,
            sync_source: SyncSource::External,
            midi_clock_out_enabled: false,
            midi_out_enabled: true,
            midi_out_internal_notes: true,
        });
        let mut runner = FakeRunner::default();
        let mut host = FakeHost::default();
        let mut control_changes = MidiControlChangeBatch::default();
        let drained: [&[u8]; 5] = [
            &[0xB0, 74, 10],
            &[0xF8],
            &[0xB0, 74, 20],
            &[0xF8],
            &[0xB0, 74, 127],
        ];

        for bytes in drained {
            match MidiControlChange::parse(bytes) {
                Some(change) => control_changes.push(change),
                None => runtime
                    .handle_midi_realtime_bytes(bytes, &mut runner, &mut host)
                    .unwrap(),
            }
        }
        for change in control_changes {
            runtime
                .dispatch(
                    RuntimeDispatchInput::HostMessage(change.into()),
                    &mut runner,
                    &mut host,
                )
                .unwrap();
        }

        assert_eq!(
            runner.seen,
            vec![
                HostMessage::MidiRealtimeClock { pulses: 1 },
                HostMessage::MidiRealtimeClock { pulses: 1 },
                HostMessage::MidiControlChange {
                    channel: 0,
                    controller: 74,
                    value: 127
                },
            ]
        );
    }
}
