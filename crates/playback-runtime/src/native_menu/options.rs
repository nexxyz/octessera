use platform_core::{BUS_COUNT as FX_BUS_COUNT, INSTRUMENT_COUNT};

pub(super) const FX_BUS_SLOT_OPTIONS: &[&str] = &[
    "none",
    "tremolo",
    "delay",
    "vibrato",
    "chorus",
    "flanger",
    "filter_lfo",
    "wah",
    "reverb",
    "glitch",
    "auto_pan",
    "duck",
    "saturator",
    "distortion",
    "bitcrusher",
    "compressor",
    "eq",
];

pub(super) const GLOBAL_FX_SLOT_OPTIONS: &[&str] = &[
    "none",
    "vinyl",
    "eq",
    "compressor",
    "saturator",
    "distortion",
];

pub(super) fn duck_source_options(bus_index: usize) -> Vec<String> {
    let mut options: Vec<String> = (0..INSTRUMENT_COUNT)
        .map(|index| format!("I{}", index + 1))
        .collect();
    options.extend(
        (0..FX_BUS_COUNT)
            .filter(|index| *index != bus_index)
            .map(|index| format!("B{}", index + 1)),
    );
    options
}
