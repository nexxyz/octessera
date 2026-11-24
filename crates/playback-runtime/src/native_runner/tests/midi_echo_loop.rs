use super::midi_note_in::keyboard_runner;
use super::*;
use crate::tests::midi_echo::midi_out_runtime;
use crate::tests::support::{set_runtime_playing, FakeHost};
use crate::{
    CoreRunner, HostAdapter, MidiNote, PlaybackRuntime, RuntimeStatus, RuntimeStatusState,
    RuntimeStoreResult,
};
use std::time::{Duration, Instant};

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
    /// Echo latency of the device on MIDI out; it sends everything back in order.
    echo_ms: u64,
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
            echo_ms: 4,
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
            if at + Duration::from_millis(self.echo_ms) > now {
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

    /// How long each note-on of key 60 on MIDI out lasted, to its note-off, in ms.
    fn midi_note_lengths(&self) -> Vec<u64> {
        let mut started = None;
        let mut lengths = Vec::new();
        for (bytes, at) in self
            .host
            .midi_messages
            .iter()
            .zip(&self.host.midi_message_at)
        {
            let (Some(at), [status, 60, velocity]) = (at, bytes.as_slice()) else {
                continue;
            };
            if status & 0xF0 == 0x90 && *velocity > 0 {
                started = Some(*at);
            } else if let Some(start) = started.take() {
                lengths.push(at.saturating_duration_since(start).as_millis() as u64);
            }
        }
        lengths
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
    // Each note lasts its One-Shot Len, or one 125 ms step where the next strike of its key
    // ends it first; an echoed note-off would cut it shorter.
    let lengths = rig.midi_note_lengths();
    assert!(
        lengths.len() >= 15 && lengths.iter().all(|length| (123..=152).contains(length)),
        "{lengths:?}"
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

/// A drum machine's steady hits through a forwarding slot all go out, at any musical rate:
/// 16ths at 120 BPM (125 ms) and 32nds (62 ms), at one fixed velocity.
#[test]
fn a_steady_fixed_velocity_source_repeating_a_key_plays_every_hit() {
    for (midi, interval) in [(true, 125), (true, 62), (false, 125), (false, 62)] {
        let mut rig = EchoRig::new(midi);
        rig.layer.playing = false;
        let press = |rig: &mut EchoRig, ms: u64, bytes: [u8; 3]| {
            rig.host.clock = Some(rig.start + Duration::from_millis(ms));
            let message: HostMessage = MidiNote::parse(&bytes).unwrap().into();
            rig.runtime
                .dispatch_host_message(message, &mut rig.layer, &mut rig.host)
                .unwrap();
        };
        for hit in 0..32 {
            press(&mut rig, hit * interval, [0x90, 60, 100]);
            press(&mut rig, hit * interval + 20, [0x80, 60, 0]);
        }
        assert_eq!(
            rig.midi_note_ons(),
            32,
            "midi={midi} {interval} ms: every hit goes out"
        );
        if !midi {
            assert_eq!(rig.keyboard_audio_notes(), 32, "every hit sounds");
        }
    }
}

#[test]
fn a_key_held_through_a_slow_thru_plays_once_and_its_echoes_stay_harmless() {
    for midi in [true, false] {
        let mut rig = EchoRig::new(midi);
        rig.layer.playing = false;
        rig.echo_ms = 300;
        let play = |rig: &mut EchoRig, bytes: [u8; 3]| {
            let message: HostMessage = MidiNote::parse(&bytes).unwrap().into();
            rig.runtime
                .dispatch_host_message(message, &mut rig.layer, &mut rig.host)
                .unwrap();
            rig.collect_sent();
        };
        play(&mut rig, [0x90, 60, 100]);
        rig.run_for(1_000);
        assert_eq!(rig.midi_note_ons(), 1, "midi={midi}: its echo is dropped");
        play(&mut rig, [0x80, 60, 0]);
        rig.run_for(2_000);
        assert_eq!(rig.midi_note_ons(), 1, "midi={midi}: nothing loops");
        if !midi {
            assert_eq!(rig.keyboard_audio_notes(), 1);
        }
        let offs = rig
            .host
            .midi_messages
            .iter()
            .filter(|bytes| bytes.len() == 3 && bytes[0] & 0xF0 == 0x80)
            .count();
        assert_eq!(offs, 1, "midi={midi}: the echoed key-up releases nothing");
    }
}

/// A tap shorter than the thru's round trip comes back after its own key-up, so it would pass
/// on for good; once it repeats eight times at that steady, fast interval its copy is no longer
/// sent.
#[test]
fn a_tap_shorter_than_a_fast_thru_round_trip_stops_looping() {
    for echo_ms in [5, 15, 30] {
        let mut rig = EchoRig::new(true);
        rig.layer.playing = false;
        rig.echo_ms = echo_ms;
        for (at, bytes) in [(0, [0x90, 60, 100]), (2, [0x80, 60, 0])] {
            rig.host.clock = Some(rig.start + Duration::from_millis(at));
            let message: HostMessage = MidiNote::parse(&bytes).unwrap().into();
            rig.runtime
                .dispatch_host_message(message, &mut rig.layer, &mut rig.host)
                .unwrap();
            rig.collect_sent();
        }
        rig.ms = 2;
        rig.run_for(2_000);
        let looped = rig.midi_note_ons();
        assert!(
            (2..=8).contains(&looped),
            "{echo_ms} ms: {looped} copies went out"
        );
        rig.run_for(1_000);
        assert_eq!(
            rig.midi_note_ons(),
            looped,
            "{echo_ms} ms: the loop has ended"
        );
    }
}
