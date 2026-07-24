#![allow(dead_code)]

use super::modulation_target::TargetValueKind;

pub(super) fn instrument_field_kind(field: &str) -> Option<(TargetValueKind, bool)> {
    let value_kind = if INSTRUMENT_ENUM_FIELDS.contains(&field) {
        TargetValueKind::Enum
    } else if INSTRUMENT_BOOL_FIELDS.contains(&field) {
        TargetValueKind::Bool
    } else if INSTRUMENT_FIELDS.contains(&field) {
        TargetValueKind::Numeric
    } else {
        return None;
    };
    Some((
        value_kind,
        field == "mixer.volume"
            || field == "mixer.panPos"
            || INSTRUMENT_ADDITIVE_FIELDS.contains(&field),
    ))
}

pub(super) fn behavior_field_kind(field: &str) -> Option<TargetValueKind> {
    if !BEHAVIOR_CONFIG_FIELDS.contains(&field) {
        return None;
    }
    Some(if BEHAVIOR_ENUM_FIELDS.contains(&field) {
        TargetValueKind::Enum
    } else {
        TargetValueKind::Numeric
    })
}

pub(super) fn pulses_field_kind(field: &str) -> Option<TargetValueKind> {
    if !PULSES_FIELDS.contains(&field) {
        return None;
    }
    Some(if PULSES_ENUM_FIELDS.contains(&field) {
        TargetValueKind::Enum
    } else if PULSES_BOOL_FIELDS.contains(&field) {
        TargetValueKind::Bool
    } else {
        TargetValueKind::Numeric
    })
}

pub(super) fn fx_field_kind(field: &str) -> Option<(TargetValueKind, bool)> {
    if !FX_FIELDS.contains(&field) {
        return None;
    }
    if field == "source" {
        return Some((TargetValueKind::Enum, true));
    }
    if matches!(field, "timeMode" | "timeNote") {
        return Some((TargetValueKind::Enum, true));
    }
    Some((
        TargetValueKind::Numeric,
        FX_EXCLUSIVE_FIELDS.contains(&field),
    ))
}

pub(super) fn sparks_field_is_exclusive(field: &str) -> bool {
    SPARKS_EXCLUSIVE_FIELDS.contains(&field)
}

pub(super) fn sparks_field_is_known(field: &str) -> bool {
    SPARKS_FIELDS.contains(&field)
}

const INSTRUMENT_FIELDS: &[&str] = &[
    "type",
    "noteBehavior",
    "mixer.route",
    "mixer.volume",
    "mixer.panPos",
    "synth.osc1.waveform",
    "synth.osc1.octave",
    "synth.osc1.levelPct",
    "synth.osc1.detuneCents",
    "synth.osc1.pulseWidthPct",
    "synth.osc2.waveform",
    "synth.osc2.octave",
    "synth.osc2.levelPct",
    "synth.osc2.detuneCents",
    "synth.osc2.pulseWidthPct",
    "synth.amp.gainPct",
    "synth.amp.velocitySensitivityPct",
    "synth.ampEnv.attackMs",
    "synth.ampEnv.decayMs",
    "synth.ampEnv.sustainPct",
    "synth.ampEnv.releaseMs",
    "synth.filter.type",
    "synth.filter.cutoffHz",
    "synth.filter.resonance",
    "synth.filter.envAmountPct",
    "synth.filter.keyTrackingPct",
    "synth.filterEnv.attackMs",
    "synth.filterEnv.decayMs",
    "synth.filterEnv.sustainPct",
    "synth.filterEnv.releaseMs",
    "sample.tuneSemis",
    "sample.selectedSlot",
    "sample.amp.gainPct",
    "sample.amp.velocitySensitivityPct",
    "sample.ampEnv.attackMs",
    "sample.ampEnv.decayMs",
    "sample.ampEnv.sustainPct",
    "sample.ampEnv.releaseMs",
    "sample.baseVelocity",
    "sample.velocityLevelsEnabled",
    "sample.velocityLevels.high",
    "sample.velocityLevels.medium",
    "sample.velocityLevels.low",
    "sample.filter.type",
    "sample.filter.cutoffHz",
    "sample.filter.resonance",
    "sample.filter.envAmountPct",
    "sample.filter.keyTrackingPct",
    "sample.filterEnv.attackMs",
    "sample.filterEnv.decayMs",
    "sample.filterEnv.sustainPct",
    "sample.filterEnv.releaseMs",
    "midi.enabled",
    "midi.channel",
    "midi.velocity",
    "midi.durationMs",
];

const INSTRUMENT_ENUM_FIELDS: &[&str] = &[
    "type",
    "noteBehavior",
    "mixer.route",
    "synth.osc1.waveform",
    "synth.osc2.waveform",
    "synth.filter.type",
    "sample.filter.type",
    "sample.selectedSlot",
];

const INSTRUMENT_BOOL_FIELDS: &[&str] = &["sample.velocityLevelsEnabled", "midi.enabled"];

const INSTRUMENT_ADDITIVE_FIELDS: &[&str] = &[
    "synth.osc1.levelPct",
    "synth.osc1.detuneCents",
    "synth.osc1.pulseWidthPct",
    "synth.osc2.levelPct",
    "synth.osc2.detuneCents",
    "synth.osc2.pulseWidthPct",
    "synth.amp.gainPct",
    "synth.amp.velocitySensitivityPct",
    "synth.ampEnv.sustainPct",
    "synth.filter.cutoffHz",
    "synth.filter.resonance",
    "synth.filter.envAmountPct",
    "synth.filter.keyTrackingPct",
    "synth.filterEnv.sustainPct",
    "sample.amp.gainPct",
    "sample.amp.velocitySensitivityPct",
    "sample.ampEnv.sustainPct",
    "sample.filter.cutoffHz",
    "sample.filter.resonance",
    "sample.filter.envAmountPct",
    "sample.filter.keyTrackingPct",
    "sample.filterEnv.sustainPct",
];

const FX_FIELDS: &[&str] = &[
    "source",
    "threshold",
    "amountPct",
    "attackMs",
    "releaseMs",
    "mixPct",
    "spreadPct",
    "timeMode",
    "timeNote",
    "timeMs",
    "feedback",
    "rateHz",
    "depthPct",
    "drive",
    "clip",
    "bits",
    "rateDiv",
    "depthMs",
    "baseMs",
    "centerHz",
    "q",
    "decay",
    "damp",
    "chancePct",
    "sliceMs",
    "thresholdDb",
    "ratio",
    "makeupDb",
    "lowGainDb",
    "midGainDb",
    "midFreqHz",
    "midQ",
    "highGainDb",
    "saturationPct",
    "cracklePct",
    "warpDepthPct",
];

const FX_EXCLUSIVE_FIELDS: &[&str] = &[
    "attackMs",
    "releaseMs",
    "timeMs",
    "timeMode",
    "timeNote",
    "rateHz",
    "rateDiv",
    "depthMs",
    "baseMs",
    "sliceMs",
    "decay",
    "bits",
];

const SPARKS_FIELDS: &[&str] = &[
    "rateHz",
    "depthPct",
    "releaseMs",
    "mixPct",
    "cutoffPct",
    "resonancePct",
    "sweepInMs",
    "sweepOutMs",
    "semitones",
    "cents",
];

const SPARKS_EXCLUSIVE_FIELDS: &[&str] = &[
    "rateHz",
    "releaseMs",
    "sweepInMs",
    "sweepOutMs",
    "semitones",
    "cents",
];

const BEHAVIOR_CONFIG_FIELDS: &[&str] = &[
    "randomCellsPerTick",
    "randomTickInterval",
    "gliderSpawnInterval",
    "spawnStep",
    "fireThreshold",
    "seedInterval",
    "randomSeedCells",
    "states",
    "threshold",
    "range",
    "treeDensityPct",
    "growChancePct",
    "spreadChancePct",
    "reseedThresholdPct",
    "lightningChancePerThousand",
    "grassGrowChancePct",
    "herbivoreReproducePct",
    "predatorReproducePct",
    "starveTicks",
    "maxAnts",
    "autoSpawnInterval",
    "spawnRatePct",
    "slideChancePct",
    "settleAge",
    "gravityDir",
    "spawnInterval",
    "spawnCount",
    "minRadius",
    "maxRadius",
    "drift",
    "current",
    "buoyancy",
    "maxBubbles",
    "maxBalls",
    "flockSize",
    "separationPct",
    "alignmentPct",
    "cohesionPct",
    "blobCount",
    "viscosityPct",
    "heatPct",
    "mergePct",
    "temperaturePct",
    "fieldStrengthPct",
    "noisePct",
    "particleCount",
    "attractionPct",
    "orbitPct",
    "repelMode",
    "windStrengthPct",
    "depositionPct",
    "erosionPct",
    "zoomRatePct",
    "driftPct",
    "iterationLimit",
    "fractalMode",
    "carvePct",
    "collapseAge",
    "walkerCount",
    "pulseShape",
    "lifespan",
    "autoPulseInterval",
    "diffusionPct",
    "fadePct",
    "dropStrength",
    "autoDropInterval",
    "splashRadius",
    "branchChancePct",
    "jitterChancePct",
    "decayTicks",
    "leaderLimit",
    "targetEdge",
    "rainPct",
    "flowPct",
    "evaporationPct",
    "feedPct",
    "killPct",
    "reactionPct",
    "dampingPct",
    "tensionPct",
    "impulseStrength",
    "autoImpulseInterval",
    "stressPct",
    "branchPct",
    "propagationPct",
    "shatterThreshold",
    "growthPct",
    "competitionPct",
    "breakawayAge",
    "growthChancePct",
    "seedStep",
    "cellLife",
    "lightBiasPct",
    "pruneAge",
    "symmetry",
    "agentCount",
    "senseDistance",
    "turnBiasPct",
    "depositAmount",
    "densityPct",
    "variationPct",
    "cycleLength",
    "seed",
    "lengthSteps",
    "quantize",
    "couplingPct",
    "frequencySpread",
    "jitterPct",
];

const BEHAVIOR_ENUM_FIELDS: &[&str] = &[
    "gravityDir",
    "repelMode",
    "fractalMode",
    "pulseShape",
    "targetEdge",
    "symmetry",
    "quantize",
];

const PULSES_FIELDS: &[&str] = &[
    "scanMode",
    "scanAxis",
    "scanUnit",
    "scanDirection",
    "scanSections",
    "eventEnabled",
    "stateNotesEnabled",
    "triggerProbabilityMode",
    "triggerProbabilityLowPct",
    "triggerProbabilityHighPct",
    "pitch.lowestNote",
    "pitch.highestNote",
    "pitch.startingNote",
    "pitch.scale",
    "pitch.root",
    "pitch.outOfRange",
    "x.pitch.enabled",
    "x.pitch.steps",
    "x.pitch.restartEachSection",
    "y.pitch.enabled",
    "y.pitch.steps",
    "y.pitch.restartEachSection",
    "x.velocity.enabled",
    "x.velocity.from",
    "x.velocity.to",
    "x.velocity.gridOffset",
    "x.velocity.curve",
    "x.filterCutoff.enabled",
    "x.filterCutoff.from",
    "x.filterCutoff.to",
    "x.filterCutoff.gridOffset",
    "x.filterCutoff.curve",
    "x.filterResonance.enabled",
    "x.filterResonance.from",
    "x.filterResonance.to",
    "x.filterResonance.gridOffset",
    "x.filterResonance.curve",
    "y.velocity.enabled",
    "y.velocity.from",
    "y.velocity.to",
    "y.velocity.gridOffset",
    "y.velocity.curve",
    "y.filterCutoff.enabled",
    "y.filterCutoff.from",
    "y.filterCutoff.to",
    "y.filterCutoff.gridOffset",
    "y.filterCutoff.curve",
    "y.filterResonance.enabled",
    "y.filterResonance.from",
    "y.filterResonance.to",
    "y.filterResonance.gridOffset",
    "y.filterResonance.curve",
    "arp.mode",
    "arp.source",
    "arp.stepIntervalSteps",
    "arp.noteLengthMs",
    "arp.gatePct",
    "arp.octaveSpread",
];

const PULSES_ENUM_FIELDS: &[&str] = &[
    "scanMode",
    "scanAxis",
    "scanUnit",
    "scanDirection",
    "scanSections",
    "triggerProbabilityMode",
    "pitch.scale",
    "pitch.root",
    "pitch.outOfRange",
    "x.velocity.curve",
    "x.filterCutoff.curve",
    "x.filterResonance.curve",
    "y.velocity.curve",
    "y.filterCutoff.curve",
    "y.filterResonance.curve",
    "arp.mode",
    "arp.source",
];

const PULSES_BOOL_FIELDS: &[&str] = &[
    "eventEnabled",
    "stateNotesEnabled",
    "x.pitch.enabled",
    "x.pitch.restartEachSection",
    "y.pitch.enabled",
    "y.pitch.restartEachSection",
    "x.velocity.enabled",
    "x.filterCutoff.enabled",
    "x.filterResonance.enabled",
    "y.velocity.enabled",
    "y.filterCutoff.enabled",
    "y.filterResonance.enabled",
];
