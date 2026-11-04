use super::{drain_midi_messages, RuntimeOutputSink, MIDI_REALTIME_BUDGET};
use crate::input::{midi_input_message, MidiMessage};
use playback_runtime::{
    HostAdapter, HostMessage, MusicalEvent, NativeRunner, NativeRunnerConfig, PlaybackRuntime,
    RuntimeAdapterError, RuntimeAudioCommand, RuntimeConfig, RuntimeIngest, RuntimePlatformRequest,
};
use std::sync::mpsc;

#[derive(Default)]
struct TestHost {
    outputs: usize,
}

impl HostAdapter for TestHost {
    fn handle_musical_event(
        &mut self,
        _event: &MusicalEvent,
        _at: Option<std::time::Instant>,
    ) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn handle_platform_effect(
        &mut self,
        _request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        Ok(Vec::new())
    }

    fn handle_audio_command(
        &mut self,
        _command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn handle_midi_message(
        &mut self,
        _bytes: &[u8],
        _at: Option<std::time::Instant>,
    ) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }
}

impl RuntimeOutputSink for TestHost {
    fn dispatch_output(
        &mut self,
        _playback: &mut PlaybackRuntime,
        _runner: &mut NativeRunner,
        _output: RuntimeIngest,
    ) -> Result<(), String> {
        self.outputs += 1;
        Ok(())
    }
}

#[test]
fn control_changes_do_not_spend_the_realtime_budget_and_coalesce_per_drain() {
    let (tx, rx) = mpsc::channel::<MidiMessage>();
    for value in 0..40 {
        tx.send(midi_input_message(&[0xB0, 74, value]).unwrap())
            .unwrap();
    }
    for _ in 0..=MIDI_REALTIME_BUDGET {
        tx.send(midi_input_message(&[0xF8]).unwrap()).unwrap();
    }
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut host = TestHost::default();

    drain_midi_messages(&rx, &mut playback, &mut runner, &mut host);

    assert_eq!(host.outputs, MIDI_REALTIME_BUDGET + 1, "32 clocks + one CC");
    assert!(matches!(rx.try_recv(), Ok(MidiMessage::Realtime { .. })));
    assert!(rx.try_recv().is_err());
}
