use super::*;

#[derive(Default)]
pub(super) struct CapturedOutput {
    pub(super) musical_event_count: usize,
    pub(super) musical_events: Vec<platform_core::MusicalEvent>,
    pub(super) audio_commands: Vec<RuntimeAudioCommand>,
    pub(super) midi_event_count: usize,
    pub(super) midi_events: Vec<platform_core::MusicalEvent>,
    pub(super) drum_hit_count: usize,
    pub(super) platform_effect_count: usize,
    pub(super) audio_command_count: usize,
    pub(super) runtime_status_count: usize,
    pub(super) sample_list_requests: Vec<(usize, usize, String)>,
    pub(super) load_preset_requests: Vec<String>,
    pub(super) saved_presets: Vec<(String, Value)>,
    pub(super) set_instrument_slot_count: usize,
    pub(super) synth_param_count: usize,
    pub(super) sample_bank_param_count: usize,
    pub(super) momentary_fx_start_count: usize,
    pub(super) momentary_fx_stop_count: usize,
    pub(super) midi_panic_count: usize,
}

impl CapturedOutput {
    pub(super) fn record(&mut self, messages: &[RunnerMessage]) {
        for message in messages {
            match message {
                RunnerMessage::MusicalEvents { events } => {
                    self.musical_event_count += events.len();
                    self.musical_events.extend(events.iter().cloned());
                }
                RunnerMessage::MidiEvents { events } => {
                    self.midi_event_count += events.len();
                    self.midi_events.extend(events.iter().cloned());
                }
                RunnerMessage::DrumHits { hits } => {
                    self.drum_hit_count += hits.len();
                }
                RunnerMessage::PlatformEffects { effects } => {
                    self.platform_effect_count += effects.len();
                    for effect in effects {
                        if let RuntimePlatformEffect::AudioCommand { command } = effect {
                            self.record_audio_command(command);
                        }
                        if matches!(effect, RuntimePlatformEffect::MidiPanic) {
                            self.midi_panic_count += 1;
                        }
                        if let RuntimePlatformEffect::SampleListRequest {
                            instrument_slot,
                            sample_slot,
                            dir,
                        } = effect
                        {
                            self.sample_list_requests.push((
                                *instrument_slot,
                                *sample_slot,
                                dir.clone(),
                            ));
                        }
                        match effect {
                            RuntimePlatformEffect::StoreLoadPreset { name } => {
                                self.load_preset_requests.push(name.clone());
                            }
                            RuntimePlatformEffect::StoreSavePreset { name, payload, .. } => {
                                self.saved_presets.push((name.clone(), payload.clone()));
                            }
                            _ => {}
                        }
                    }
                }
                RunnerMessage::AudioCommands { commands } => {
                    self.audio_command_count += commands.len();
                    for command in commands {
                        self.record_audio_command(command);
                    }
                }
                RunnerMessage::RuntimeStatus { .. } => {
                    self.runtime_status_count += 1;
                }
                RunnerMessage::Snapshot { .. }
                | RunnerMessage::OledFrame { .. }
                | RunnerMessage::RuntimeConfigChanged { .. }
                | RunnerMessage::PresentedRuntimeErrorDismissed => {}
            }
        }
    }

    fn record_audio_command(&mut self, command: &RuntimeAudioCommand) {
        self.audio_commands.push(command.clone());
        match command {
            RuntimeAudioCommand::SetInstrumentSlot { .. } => self.set_instrument_slot_count += 1,
            RuntimeAudioCommand::SetSynthParam { .. } => self.synth_param_count += 1,
            RuntimeAudioCommand::SetSampleBankParam { .. } => self.sample_bank_param_count += 1,
            RuntimeAudioCommand::MomentaryFxStart { .. } => self.momentary_fx_start_count += 1,
            RuntimeAudioCommand::MomentaryFxStop { .. } => self.momentary_fx_stop_count += 1,
            RuntimeAudioCommand::SetAudioConfig { .. }
            | RuntimeAudioCommand::SetDspConfig { .. }
            | RuntimeAudioCommand::SetMasterVolume { .. }
            | RuntimeAudioCommand::SetInstrumentMixer { .. }
            | RuntimeAudioCommand::SetFmParam { .. }
            | RuntimeAudioCommand::SetPluckParam { .. }
            | RuntimeAudioCommand::SetDrumParam { .. }
            | RuntimeAudioCommand::SetFxBusMixer { .. }
            | RuntimeAudioCommand::SetFxBusParam { .. }
            | RuntimeAudioCommand::SetFxBusSlot { .. }
            | RuntimeAudioCommand::SetGlobalFxSlot { .. }
            | RuntimeAudioCommand::SetGlobalFxParam { .. }
            | RuntimeAudioCommand::MomentaryFxUpdate { .. }
            | RuntimeAudioCommand::SamplePreview { .. } => {}
        }
    }
}
