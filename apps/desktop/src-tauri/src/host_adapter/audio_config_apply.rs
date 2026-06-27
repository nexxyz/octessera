use super::DesktopPlaybackHostAdapter;
use crate::audio_config::{
    build_audio_slot_configs, decode_sample_file, parse_voice_stealing_mode, sample_bank_signature,
    sample_banks, synth_payload, AudioInstrumentsConfig,
};
use crate::samples::resolve_sample_file;
use crate::types::QueuedAudioEvent;

impl DesktopPlaybackHostAdapter {
    pub(super) fn handle_full_audio_config(
        &mut self,
        config: serde_json::Value,
    ) -> Result<(), String> {
        let config = serde_json::from_value::<AudioInstrumentsConfig>(config)
            .map_err(|e| format!("invalid audio config payload: {e}"))?;
        let (next_slots, _) = build_audio_slot_configs(&config.instruments);
        if let Ok(mut slots) = self.audio.synth_slots.lock() {
            *slots = next_slots;
        }
        let next_sample_signature = sample_bank_signature(&config);
        let should_update_sample_banks = {
            let mut current = self
                .audio
                .sample_bank_signature
                .lock()
                .map_err(|_| "sample bank signature lock failed".to_string())?;
            if *current == next_sample_signature {
                false
            } else {
                *current = next_sample_signature;
                true
            }
        };
        let next_sample_banks = if should_update_sample_banks {
            let mut cache = self
                .audio
                .sample_cache
                .lock()
                .map_err(|_| "sample cache poisoned".to_string())?;
            Some(sample_banks(&config, resolve_sample_file, |path| {
                if let Some(buffer) = cache.get(path) {
                    return Some(buffer.clone());
                }
                let buffer = decode_sample_file(path)?;
                cache.insert(path.to_string(), buffer.clone());
                Some(buffer)
            }))
        } else {
            None
        };
        self.audio
            .trigger_tx
            .send(QueuedAudioEvent::SetInstruments(synth_payload(&config)))
            .map_err(|e| format!("audio queue send failed: {e}"))?;
        if let Some(next_sample_banks) = next_sample_banks {
            self.audio
                .trigger_tx
                .send(QueuedAudioEvent::SetSampleBanks(next_sample_banks))
                .map_err(|e| format!("audio queue send failed: {e}"))?;
        }
        if let Some(mode) = &config.voice_stealing_mode {
            self.audio
                .trigger_tx
                .send(QueuedAudioEvent::SetVoiceStealingMode(
                    parse_voice_stealing_mode(mode),
                ))
                .map_err(|e| format!("audio queue send failed: {e}"))?;
        }
        Ok(())
    }
}
