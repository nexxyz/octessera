use crate::native_menu::format_values::format_display_value;

fn shown(key: &str, raw: &str) -> String {
    format_display_value(Some(key), raw)
}

#[test]
fn values_show_their_unit_without_a_space_and_off_where_zero_switches_off() {
    for (key, raw, expected) in [
        ("mixer.buses.0.slot1.params.timeMode", "note", "note"),
        ("mixer.buses.0.slot1.params.timeNote", "1/8", "1/8"),
        ("mixer.buses.0.slot1.params.feedback", "49", "49"),
        ("mixer.buses.0.slot1.params.centerHz", "1600", "1.6kHz"),
        ("mixer.buses.0.slot1.params.rateHz", "450", "4.5Hz"),
        ("mixer.buses.0.slot1.params.ratio", "8", "4:1"),
        ("mixer.buses.0.slot1.params.baseMs", "80", "8ms"),
        ("play.fx.params.rateHz", "8", "8Hz"),
        ("play.fx.params.releaseMs", "2500", "2.5s"),
        ("play.fx.params.mixPct", "60", "60%"),
        ("play.fx.target", "fx_bus_2", "fxb2"),
        ("instruments.0.pluck.bodyFrequencyHz", "500", "500Hz"),
        ("dsp.busIdleThreshold", "-140", "-140dB"),
        ("dsp.busIdleThreshold", "exact", "exact"),
        ("dsp.workerWarningThreshold", "85", "85%"),
        ("transport.bpm", "120", "120bpm"),
        ("recording.maxMinutes", "30", "30min"),
        ("screenSleepSeconds", "0", "Off"),
        ("screenSleepSeconds", "300", "5min"),
        ("dimTimerSeconds", "90", "1m30s"),
        (
            "layers.0.build.behaviorConfig.autoSpawnInterval",
            "0",
            "Off",
        ),
        ("layers.0.build.behaviorConfig.randomSeedCells", "0", "Off"),
        ("layers.0.link.mapping.activate.retriggerCount", "0", "Off"),
        ("play.xy.smoothingMs", "80", "80ms"),
        ("instruments.0.drum.voices.0.sweepSemis", "24", "24st"),
        ("instruments.0.drum.voices.0.tuneSemis", "-3", "-3st"),
        ("instruments.0.sample.tuneSemis", "12", "12st"),
        ("instruments.0.tile.tuneSemis", "0", "0st"),
        ("instruments.0.tile.tuneCents", "-25", "-25c"),
        ("instruments.0.synth.osc1.detuneCents", "7", "7c"),
        ("play.fx.params.semitones", "-12", "-12st"),
        ("play.fx.params.cents", "50", "50c"),
    ] {
        assert_eq!(shown(key, raw), expected, "{key} = {raw}");
    }
}

#[test]
fn stored_ids_show_their_display_labels() {
    for (key, raw, expected) in [
        (
            "layers.0.link.mapping.activate.action",
            "note_on",
            "note on",
        ),
        ("layers.0.link.arp.mode", "outside_in", "outside in"),
        ("layers.0.link.triggerProbabilityMode", "zero", "0%"),
        ("hdmi.mode", "live-grid", "Live Grid"),
        ("instruments.0.type", "synth", "Synth"),
        ("mixer.buses.0.slot1.type", "filter_lfo", "Filter LFO"),
        ("mixer.master.slots.0.type", "compressor", "Compressor"),
        ("play.fx.type", "filter_sweep", "filter sweep"),
    ] {
        assert_eq!(shown(key, raw), expected, "{key} = {raw}");
    }
}
