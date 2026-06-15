use realtime_engine::synth::{
    default_synth_config, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    DEFAULT_PAN_POSITIONS, INSTRUMENT_SLOT_COUNT,
};
use rodio::{OutputStream, Sink};
use rodio_engine_source::{EngineEvent, EngineSource};
use std::sync::mpsc;

pub struct AudioManager {
    _stream: OutputStream,
    _sink: Sink,
    engine_tx: mpsc::Sender<EngineEvent>,
}

impl AudioManager {
    pub fn new() -> Result<Self, String> {
        let (stream, handle) = OutputStream::try_default()
            .map_err(|e| format!("failed to create audio stream: {e}"))?;
        let sink = Sink::try_new(&handle).map_err(|e| format!("failed to create sink: {e}"))?;
        let (engine_tx, engine_rx) = mpsc::channel::<EngineEvent>();
        sink.append(EngineSource::new(engine_rx, 48_000));
        sink.play();
        let _ = engine_tx.send(EngineEvent::SetInstruments(default_pi_instruments()));
        Ok(Self {
            _stream: stream,
            _sink: sink,
            engine_tx,
        })
    }

    pub fn send(&self, event: EngineEvent) -> Result<(), String> {
        self.engine_tx
            .send(event)
            .map_err(|e| format!("audio event send failed: {e}"))
    }
}

fn default_pi_instruments() -> InstrumentsConfig {
    let synth = default_synth_config();
    InstrumentsConfig {
        instruments: (0..INSTRUMENT_SLOT_COUNT)
            .map(|idx| InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth,
                mixer: Some(InstrumentMixerConfig {
                    route: "direct".to_string(),
                    pan_pos: idx.min(DEFAULT_PAN_POSITIONS - 1),
                    volume: 100.0,
                }),
            })
            .collect(),
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}
