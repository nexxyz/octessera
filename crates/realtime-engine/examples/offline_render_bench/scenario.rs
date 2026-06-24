use crate::sample_data::{sample_bank, sample_buffer};
use crate::SECONDS;
use realtime_engine::synth::{SynthEngine, INSTRUMENT_SLOT_COUNT};

#[derive(Clone, Copy)]
pub(crate) enum Scenario {
    Mixed,
    Synth,
    Fx,
    Dense,
    FxIsolation(Option<IsolatedFx>),
    MasterIsolation(Option<IsolatedFx>),
    Sample,
    SampleFx,
    SamplePreview,
}

impl Scenario {
    pub(crate) fn from_args(arg: Option<String>) -> Result<Self, String> {
        let name = arg.as_deref().unwrap_or("mixed");
        match name {
            "mixed" => Ok(Self::Mixed),
            "synth" => Ok(Self::Synth),
            "fx" => Ok(Self::Fx),
            "dense" => Ok(Self::Dense),
            "fx-none" => Ok(Self::FxIsolation(None)),
            "fx-delay" => Ok(Self::FxIsolation(Some(IsolatedFx::Delay))),
            "fx-chorus" => Ok(Self::FxIsolation(Some(IsolatedFx::Chorus))),
            "fx-filter-lfo" => Ok(Self::FxIsolation(Some(IsolatedFx::FilterLfo))),
            "fx-tremolo" => Ok(Self::FxIsolation(Some(IsolatedFx::Tremolo))),
            "fx-reverb" => Ok(Self::FxIsolation(Some(IsolatedFx::Reverb))),
            "fx-auto-pan" => Ok(Self::FxIsolation(Some(IsolatedFx::AutoPan))),
            "fx-saturator" => Ok(Self::FxIsolation(Some(IsolatedFx::Saturator))),
            "fx-compressor" => Ok(Self::FxIsolation(Some(IsolatedFx::Compressor))),
            "fx-eq" => Ok(Self::FxIsolation(Some(IsolatedFx::Eq))),
            "fx-vinyl" => Ok(Self::FxIsolation(Some(IsolatedFx::Vinyl))),
            "master-none" => Ok(Self::MasterIsolation(None)),
            "master-compressor" => Ok(Self::MasterIsolation(Some(IsolatedFx::Compressor))),
            "master-eq" => Ok(Self::MasterIsolation(Some(IsolatedFx::Eq))),
            "master-vinyl" => Ok(Self::MasterIsolation(Some(IsolatedFx::Vinyl))),
            "sample" => Ok(Self::Sample),
            "sample-fx" => Ok(Self::SampleFx),
            "sample-preview" => Ok(Self::SamplePreview),
            other => Err(format!(
                "unknown scenario '{other}'. usage: offline_render_bench [{}]",
                SCENARIOS.join("|")
            )),
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Mixed => "mixed",
            Self::Synth => "synth",
            Self::Fx => "fx",
            Self::Dense => "dense",
            Self::FxIsolation(None) => "fx-none",
            Self::FxIsolation(Some(fx)) => fx.fx_scenario_name(),
            Self::MasterIsolation(None) => "master-none",
            Self::MasterIsolation(Some(fx)) => fx.master_scenario_name(),
            Self::Sample => "sample",
            Self::SampleFx => "sample-fx",
            Self::SamplePreview => "sample-preview",
        }
    }

    pub(crate) fn setup(self, engine: &mut SynthEngine) {
        match self {
            Self::Sample | Self::SampleFx => engine.set_sample_banks(vec![sample_bank()]),
            Self::SamplePreview => engine.preview_sample(0, sample_buffer(), 96),
            _ => {}
        }
    }

    pub(crate) fn schedule(self, engine: &mut SynthEngine, frame: usize) {
        match self {
            Self::Mixed => schedule_note(engine, frame, 6_000, 450, 48, 24),
            Self::Synth => schedule_note(engine, frame, 2_000, 1_000, 48, 36),
            Self::Fx => schedule_note(engine, frame, 3_000, 900, 48, 24),
            Self::Dense => schedule_note(engine, frame, 250, 2_000, 36, 48),
            Self::FxIsolation(_) | Self::MasterIsolation(_) => {
                schedule_sustained_note(engine, frame)
            }
            Self::Sample | Self::SampleFx => schedule_sample_note(engine, frame),
            Self::SamplePreview => {}
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum IsolatedFx {
    Delay,
    Chorus,
    FilterLfo,
    Tremolo,
    Reverb,
    AutoPan,
    Saturator,
    Compressor,
    Eq,
    Vinyl,
}

impl IsolatedFx {
    pub(crate) fn fx_scenario_name(self) -> &'static str {
        match self {
            Self::Delay => "fx-delay",
            Self::Chorus => "fx-chorus",
            Self::FilterLfo => "fx-filter-lfo",
            Self::Tremolo => "fx-tremolo",
            Self::Reverb => "fx-reverb",
            Self::AutoPan => "fx-auto-pan",
            Self::Saturator => "fx-saturator",
            Self::Compressor => "fx-compressor",
            Self::Eq => "fx-eq",
            Self::Vinyl => "fx-vinyl",
        }
    }

    pub(crate) fn master_scenario_name(self) -> &'static str {
        match self {
            Self::Compressor => "master-compressor",
            Self::Eq => "master-eq",
            Self::Vinyl => "master-vinyl",
            _ => unreachable!(),
        }
    }
}

const SCENARIOS: &[&str] = &[
    "mixed",
    "synth",
    "fx",
    "dense",
    "fx-none",
    "fx-delay",
    "fx-chorus",
    "fx-filter-lfo",
    "fx-tremolo",
    "fx-reverb",
    "fx-auto-pan",
    "fx-saturator",
    "fx-compressor",
    "fx-eq",
    "fx-vinyl",
    "master-none",
    "master-compressor",
    "master-eq",
    "master-vinyl",
    "sample",
    "sample-fx",
    "sample-preview",
];

fn schedule_note(
    engine: &mut SynthEngine,
    frame: usize,
    interval_frames: usize,
    duration_ms: u32,
    base_note: u8,
    note_span: usize,
) {
    if frame.is_multiple_of(interval_frames) {
        let step = frame / interval_frames;
        let slot = (step % INSTRUMENT_SLOT_COUNT) as u8;
        let note = base_note + (step % note_span) as u8;
        engine.note_on(slot, note, 96, duration_ms);
    }
}

fn schedule_sustained_note(engine: &mut SynthEngine, frame: usize) {
    if frame == 0 {
        engine.note_on(0, 60, 96, ((SECONDS + 1) * 1_000) as u32);
    }
}

fn schedule_sample_note(engine: &mut SynthEngine, frame: usize) {
    if frame == 0 {
        engine.note_on(0, 36, 96, ((SECONDS + 1) * 1_000) as u32);
    }
}
