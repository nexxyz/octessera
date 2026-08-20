use super::{instrument_audio_payload, json, NativeRunner, Value, PAN_POSITION_COUNT};

impl NativeRunner {
    pub(super) fn instrument_audio_config(&self, index: usize) -> Option<Value> {
        self.instruments.get(index).map(instrument_audio_payload)
    }

    pub(super) fn audio_snapshot_payload(&self) -> Value {
        json!({
            "instruments": self.instruments.iter().map(|instrument| {
                instrument_audio_payload(instrument)
            }).collect::<Vec<_>>(),
            "mixer": self.mixer_payload(),
            "panPositions": PAN_POSITION_COUNT,
        })
    }
}
