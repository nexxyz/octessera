#![allow(dead_code)]

use super::modulation_target::TargetValueKind;

#[path = "modulation_instrument_target_fields.rs"]
mod instrument_fields;
use self::instrument_fields::{
    INSTRUMENT_ADDITIVE_FIELDS, INSTRUMENT_BOOL_FIELDS, INSTRUMENT_ENUM_FIELDS, INSTRUMENT_FIELDS,
};

pub(super) fn instrument_field_kind(field: &str) -> Option<(TargetValueKind, bool)> {
    if super::drum_config::voice_numeric_field(field).is_some() {
        return Some((TargetValueKind::Numeric, false));
    }
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

pub(super) fn link_field_kind(field: &str) -> Option<TargetValueKind> {
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
    if matches!(field, "source" | "sourceTap") {
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

pub(super) fn play_field_is_exclusive(field: &str) -> bool {
    PLAY_EXCLUSIVE_FIELDS.contains(&field)
}

pub(super) fn play_field_is_known(field: &str) -> bool {
    PLAY_FIELDS.contains(&field)
}

const FX_FIELDS: &[&str] = &[
    "source",
    "sourceTap",
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
    "rateDiv",
    "depthMs",
    "baseMs",
    "sliceMs",
    "decay",
    "bits",
];

const PLAY_FIELDS: &[&str] = &[
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
    "slideInMs",
    "slideOutMs",
];

const PLAY_EXCLUSIVE_FIELDS: &[&str] = &[
    "rateHz",
    "releaseMs",
    "sweepInMs",
    "sweepOutMs",
    "semitones",
    "cents",
    "slideInMs",
    "slideOutMs",
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
