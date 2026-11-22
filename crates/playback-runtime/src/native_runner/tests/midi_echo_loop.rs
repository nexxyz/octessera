use super::midi_note_in::keyboard_runner;
use super::*;
use crate::tests::midi_echo::midi_out_runtime;
use crate::tests::support::{set_runtime_playing, FakeHost};
use crate::{
    CoreRunner, HostAdapter, MidiNote, PlaybackRuntime, RuntimeStatus, RuntimeStatusState,
    RuntimeStoreResult,
};
use std::time::{Duration, Instant};

/// Echo latency of the device on MIDI out; it sends everything back in order.
const ECHO_MS: u64 = 4;
/// One 1/16 step at 120 BPM, shorter than the default 150 ms One-Shot Len.
const STEP_PULSES: u64 = 6;
const ONE_SHOT_MS: u32 = 150;

/// A layer on slot 1 repeating C4 every 1/16, with the real runner playing the keyboard notes
/// that come back on MIDI In.
struct RepeatingLayer {
    keys: NativeRunner,
    playing: bool,
    pulse: u64,
    midi: bool,
}

impl RepeatingLayer {
    fn new(midi: bool) -> Self {
        let mut keys = keyboard_runner("keys");
        keys.midi.output_id = Some("out".into());
        let instrument = &mut keys.instruments[0];
        if midi {
            instrument.kind = InstrumentType::Midi;
            instrument.midi_enabled = true;
            instrument.midi_channel = 1;
        } else {
            instrument.kind = InstrumentType::Synth;
        }
        Self {
            keys,
            playing: true,
            pulse: 0,
            midi,
        }
    }

    fn status(&self) -> RunnerMessage {
        RunnerMessage::RuntimeStatus {
            status: RuntimeStatus {
                state: if self.playing {
                    RuntimeStatusState::Running
                } else {
                    RuntimeStatusState::Idle
                },
                transport: if self.playing {
                    RuntimeTransportState::Playing
                } else {
                    RuntimeTransportState::Stopped
                },
                current_ppqn_pulse: self.pulse,
                pending_resync: false,
                sync_source: SyncSource::Internal,
                message: None,
                error: None,
            },
        }
    }
}

impl CoreRunner for RepeatingLayer {
    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        match message {
            HostMessage::TransportPulseStep { pulses, .. } if self.playing => {
                let mut events = Vec::new();
                for _ in 0..pulses {
                    if self.pulse.is_multiple_of(STEP_PULSES) {
                        events.push(MusicalEvent::NoteOn {
                            channel: 0,
                            note: 60,
                            velocity: 100,
                            duration_ms: Some(ONE_SHOT_MS),
                        });
                    }
                    self.pulse += 1;
                }
                let mut messages = Vec::new();
                if !events.is_empty() {
                    messages.push(if self.midi {
                        RunnerMessage::MidiEvents { events }
                    } else {
                        RunnerMessage::MusicalEvents { events }
                    });
                }
                messages.push(self.status());
                Ok(messages)
            }
            HostMessage::TransportStop => {
                self.playing = false;
                Ok(vec![self.status()])
            }
            HostMessage::MidiNote { .. } => Ok(self
                .keys
                .send(message)?
                .into_iter()
                .filter(|message| {
                    matches!(
                        message,
                        RunnerMessage::MidiEvents { .. } | RunnerMessage::MusicalEvents { .. }
                    )
                })
                .collect()),
            _ => Ok(Vec::new()),
        }
    }

    fn send_system_store_result(
        &mut self,
        _message: HostMessage,
    ) -> Result<(Vec<RunnerMessage>, Option<RuntimeStoreResult>), String> {
        Ok((Vec::new(), None))
    }
}

/// Runs the runtime in 1 ms ticks with an echoing device on MIDI out.
struct EchoRig {
    runtime: PlaybackRuntime,
    layer: RepeatingLayer,
    host: FakeHost,
    start: Instant,
    ms: u64,
    seen: usize,
    in_flight: std::collections::VecDeque<(Instant, [u8; 3])>,
}

impl EchoRig {
    fn new(midi: bool) -> Self {
        let mut runtime = midi_out_runtime();
        let start = Instant::now();
        let mut host = FakeHost {
            clock: Some(start),
            ..FakeHost::default()
        };
        set_runtime_playing(&mut runtime, &mut host);
        host.midi_message_at.clear();
        Self {
            runtime,
            layer: RepeatingLayer::new(midi),
            host,
            start,
            ms: 0,
            seen: 0,
            in_flight: Default::default(),
        }
    }

    fn run_for(&mut self, ms: u64) {
        for _ in 0..ms {
            self.ms += 1;
            let now = self.start + Duration::from_millis(self.ms);
            self.host.clock = Some(now);
            self.runtime
                .advance(1, &mut self.layer, &mut self.host)
                .unwrap();
            self.echo_due(now);
        }
    }

    fn stop(&mut self) {
        self.runtime
            .dispatch_host_message(HostMessage::TransportStop, &mut self.layer, &mut self.host)
            .unwrap();
        self.collect_sent();
    }

    fn collect_sent(&mut self) {
        let now = self.host.now();
        for index in self.seen..self.host.midi_messages.len() {
            if let Ok(bytes) = <[u8; 3]>::try_from(self.host.midi_messages[index].as_slice()) {
                if bytes[0] & 0xE0 == 0x80 {
                    let at = self.host.midi_message_at[index].unwrap_or(now);
                    self.in_flight.push_back((at, bytes));
                }
            }
        }
        self.seen = self.host.midi_messages.len();
    }

    fn echo_due(&mut self, now: Instant) {
        self.collect_sent();
        while let Some(&(at, bytes)) = self.in_flight.front() {
            if at + Duration::from_millis(ECHO_MS) > now {
                break;
            }
            self.in_flight.pop_front();
            let message: HostMessage = MidiNote::parse(&bytes).unwrap().into();
            self.runtime
                .dispatch_host_message(message, &mut self.layer, &mut self.host)
                .unwrap();
            self.collect_sent();
        }
    }

    fn midi_note_ons(&self) -> usize {
        self.host
            .midi_messages
            .iter()
            .filter(|bytes| bytes.len() == 3 && bytes[0] & 0xF0 == 0x90)
            .count()
    }

    /// Notes the slot played from MIDI In, held until their note-off.
    fn keyboard_audio_notes(&self) -> usize {
        self.host
            .musical_events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    MusicalEvent::NoteOn {
                        duration_ms: None,
                        ..
                    }
                )
            })
            .count()
    }
}

#[test]
fn an_echoing_device_on_a_midi_slot_never_loops_a_repeated_pitch_back_in() {
    let mut rig = EchoRig::new(true);
    rig.run_for(2_000);
    assert_eq!(
        rig.midi_note_ons(),
        16,
        "only the layer's own 16 steps go out; no echo comes back in as a key"
    );

    rig.stop();
    let at_stop = rig.midi_note_ons();
    rig.run_for(1_000);
    assert_eq!(
        rig.midi_note_ons(),
        at_stop,
        "nothing keeps playing after Stop"
    );
}

#[test]
fn an_echoing_device_never_doubles_a_repeated_pitch_on_an_internal_slot() {
    let mut rig = EchoRig::new(false);
    rig.run_for(2_000);
    assert_eq!(
        rig.keyboard_audio_notes(),
        0,
        "the echoes of the copies Send MIDI sends never play the synth again"
    );
    assert_eq!(rig.midi_note_ons(), 16, "one copy per step");

    rig.stop();
    let at_stop = rig.midi_note_ons();
    rig.run_for(1_000);
    assert_eq!(rig.midi_note_ons(), at_stop);
    assert_eq!(rig.keyboard_audio_notes(), 0);
}

#[test]
fn a_fixed_velocity_pad_repeating_a_key_plays_every_hit_without_an_echoing_device() {
    for midi in [true, false] {
        let mut rig = EchoRig::new(midi);
        rig.layer.playing = false;
        let press = |rig: &mut EchoRig, ms: u64, bytes: [u8; 3]| {
            rig.host.clock = Some(rig.start + Duration::from_millis(ms));
            let message: HostMessage = MidiNote::parse(&bytes).unwrap().into();
            rig.runtime
                .dispatch_host_message(message, &mut rig.layer, &mut rig.host)
                .unwrap();
        };
        for hit in 0..10 {
            press(&mut rig, hit * 40, [0x90, 60, 100]);
            press(&mut rig, hit * 40 + 20, [0x80, 60, 0]);
        }
        assert_eq!(rig.midi_note_ons(), 10, "midi={midi}: every hit goes out");
        if !midi {
            assert_eq!(rig.keyboard_audio_notes(), 10, "every hit sounds");
        }
    }
}
