use realtime_engine::synth::REVERB_COMB_LENGTHS;

pub(crate) fn format_display_value(key: Option<&str>, value: impl ToString) -> String {
    let raw = value.to_string();
    let Some(key) = key else {
        return raw;
    };
    if key.starts_with("instruments.") && key.ends_with(".fm.ratio") {
        return match raw.as_str() {
            "0.5" => "1:2".into(),
            "1" => "1:1".into(),
            "2" => "2:1".into(),
            "3" => "3:1".into(),
            "4" => "4:1".into(),
            "5" => "5:1".into(),
            "6" => "6:1".into(),
            "8" => "8:1".into(),
            _ => raw,
        };
    }
    if key.starts_with("instruments.") && key.ends_with(".fm.ratioFineCents") {
        return format!("{:+}c", raw.parse::<i32>().unwrap_or(0));
    }
    if let Some(label) = enum_display_label(key, &raw) {
        return label.into();
    }
    if key.starts_with("instruments.") && key.ends_with(".midiDrum.tileNote") {
        let note = raw.parse::<i32>().unwrap_or(0).clamp(0, 127);
        let shown = format!("{note} {}", note_name(note));
        return match super::voice::gm_percussion_name(note) {
            Some(name) => format!("{shown} {name}"),
            None => shown,
        };
    }
    if key.starts_with("instruments.") && key.contains(".drum.voices.") && key.ends_with(".sound") {
        return match raw.as_str() {
            "kick" => "Kick".into(),
            "snare" => "Snare".into(),
            "closed_hat" => "Closed Hat".into(),
            "open_hat" => "Open Hat".into(),
            "low_tom" => "Low Tom".into(),
            "high_tom" => "High Tom".into(),
            "clap" => "Clap".into(),
            "rim" => "Rim".into(),
            _ => raw,
        };
    }
    if let Some(param) = key.strip_prefix("play.fx.params.") {
        return format_play_fx_param(param, &raw);
    }
    if key == "play.fx.target" {
        if let Some(slot) = raw.strip_prefix("instrument_") {
            return format!("I{slot}");
        }
        return compact_route_value(&raw);
    }
    if key == "play.xy.release" {
        return match raw.as_str() {
            "sample-hold" => "hold".into(),
            "reset-center" => "center".into(),
            _ => raw,
        };
    }
    if key.ends_with(".arp.source") && raw == "simultaneous" {
        return "together".into();
    }
    if key.contains(".sample.slots.") && key.ends_with(".loop") && raw == "pingpong" {
        return "ping-pong".into();
    }
    if key == "midi.sendInstruments" {
        return match raw.as_str() {
            "midi" => "MIDI inst.".into(),
            "all" => "all inst.".into(),
            _ => raw,
        };
    }
    if key == "sound.optimizeFor" {
        return match raw.as_str() {
            "latency" => "Lat".into(),
            "capacity" => "Cap".into(),
            _ => raw,
        };
    }
    if key == "sound.voiceStealingMode" {
        return match raw.as_str() {
            "fixed12" => "12".into(),
            "fixed16" => "16".into(),
            "auto-soft" => "soft".into(),
            "auto-balanced" => "bal".into(),
            "auto-hard" => "hard".into(),
            _ => raw,
        };
    }
    if raw == "0" && zero_means_off(key) {
        return "Off".into();
    }
    if let Some(text) = format_system_value(key, &raw) {
        return text;
    }
    if key.ends_with("panPos") {
        return format_pan_position(raw.parse::<i32>().unwrap_or(16));
    }
    if key.ends_with("mixer.route") {
        return compact_route_value(&raw);
    }
    if key.ends_with("pitch.lowestNote")
        || key.ends_with("pitch.highestNote")
        || key.ends_with("pitch.startingNote")
    {
        return format_note_with_midi(raw.parse::<i32>().unwrap_or(60));
    }
    if key.ends_with("pitch.scale") {
        return platform_core::note_set_by_id(&raw)
            .map(|note_set| note_set.compact_label.to_string())
            .unwrap_or(raw);
    }
    if key.ends_with(".params.source") {
        return raw;
    }
    if key.ends_with(".params.sourceTap") {
        return match raw.as_str() {
            "pre" => "Pre".into(),
            "post" => "Post".into(),
            _ => raw,
        };
    }
    if key.contains(".params.") {
        return match raw.parse::<i32>() {
            Ok(value) => format_fx_param_display(key, value),
            Err(_) => raw,
        };
    }
    if key.ends_with("Pct") || key.ends_with("Percent") {
        return format!("{}%", raw.parse::<i32>().unwrap_or(0));
    }
    if key.ends_with("Ms") {
        return format_ms(raw.parse::<i32>().unwrap_or(0));
    }
    if key.ends_with("Hz") && !key.ends_with("cutoffHz") {
        return format_hz(raw.parse().unwrap_or(0));
    }
    if key.ends_with("Semis") {
        return format!("{}st", raw.parse::<i32>().unwrap_or(0));
    }
    if key.ends_with("Cents") {
        return format!("{}c", raw.parse::<i32>().unwrap_or(0));
    }
    raw
}

/// Rows whose 0 switches their job off: auto-spawn intervals and counts, retriggers, timers.
fn zero_means_off(key: &str) -> bool {
    key.ends_with("Interval")
        || key.ends_with("randomCellsPerTick")
        || key.ends_with("randomSeedCells")
        || key.ends_with(".retriggerCount")
        || matches!(
            key,
            "play.xy.smoothingMs" | "screenSleepSeconds" | "dimTimerSeconds"
        )
}

fn format_system_value(key: &str, raw: &str) -> Option<String> {
    let text = match key {
        "screenSleepSeconds" | "dimTimerSeconds" => format_seconds(raw.parse().unwrap_or(0)),
        "recording.maxMinutes" => format!("{raw}min"),
        "transport.bpm" => format!("{raw}bpm"),
        "dsp.busIdleThreshold" if raw != "exact" => format!("{raw}dB"),
        "dsp.workerWarningThreshold" | "buttonBrightness" | "gridBrightness" => {
            format!("{raw}%")
        }
        _ => return None,
    };
    Some(text)
}

fn format_seconds(seconds: i32) -> String {
    match (seconds / 60, seconds % 60) {
        (0, seconds) => format!("{seconds}s"),
        (minutes, 0) => format!("{minutes}min"),
        (minutes, seconds) => format!("{minutes}m{seconds}s"),
    }
}

fn format_hz(hz: i32) -> String {
    if hz >= 1000 {
        format!("{}kHz", format_fixed(f64::from(hz) / 1000.0, 1))
    } else {
        format!("{hz}Hz")
    }
}

fn format_ms(ms: i32) -> String {
    if ms.abs() >= 1000 {
        format!("{:.1}s", ms as f32 / 1000.0)
    } else {
        format!("{ms}ms")
    }
}

/// A Play FX parameter: stored whole, in the unit its key names.
fn format_play_fx_param(param: &str, raw: &str) -> String {
    let value = raw.parse::<i32>().unwrap_or(0);
    if param.ends_with("Note") {
        return crate::native_runner::play_fx_config::play_fx_note_unit(i64::from(value)).into();
    }
    if param.ends_with("Hz") {
        format!("{value}Hz")
    } else if param.ends_with("Pct") {
        format!("{value}%")
    } else if param.ends_with("Ms") {
        format_ms(value)
    } else if param == "semitones" {
        format!("{value}st")
    } else if param == "cents" {
        format!("{value}c")
    } else {
        value.to_string()
    }
}

/// The display label of a stored enum id; the saved value stays the id.
fn enum_display_label(key: &str, raw: &str) -> Option<&'static str> {
    if key.starts_with("instruments.") && key.ends_with(".type") && !key.contains(".filter.") {
        return Some(instrument_kind_label(raw));
    }
    if key.starts_with("mixer.") && key.ends_with(".type") {
        return Some(fx_type_label(raw)).filter(|label| !label.is_empty());
    }
    let label = match (key, raw) {
        ("play.fx.type", "filter_sweep") => "filter sweep",
        ("play.fx.type", "pitch_shift") => "pitch",
        ("play.fx.type", "tape_stop") => "tape stop",
        ("play.fx.type", "trance_gate") => "gate",
        ("play.fx.type", "dub_throw") => "dub",
        ("play.fx.type", "hp_riser") => "HP riser",
        ("hdmi.mode", "Terminal") => "Terminal",
        ("hdmi.mode", "live-grid") => "Live Grid",
        ("hdmi.mode", "plain-grid") => "Plain Grid",
        ("hdmi.mode", "active-behavior") => "Behavior",
        ("hdmi.mode", "cycle-behaviors") => "Cycle",
        (_, "note_on") if key.contains(".link.mapping.") => "note on",
        (_, "note_off") if key.contains(".link.mapping.") => "note off",
        (_, "zero") if key.ends_with(".triggerProbabilityMode") => "0%",
        (_, "full") if key.ends_with(".triggerProbabilityMode") => "100%",
        (_, "outside_in") if key.ends_with(".arp.mode") => "outside in",
        (_, "octave_spread") if key.ends_with(".arp.mode") => "octave spread",
        (_, "chord_strike") if key.ends_with(".arp.mode") => "chord strike",
        _ => return None,
    };
    Some(label)
}

/// An instrument type's display name, also its auto label.
pub(crate) fn instrument_kind_label(kind: &str) -> &'static str {
    match kind {
        "synth" => "Synth",
        "fm" => "FM",
        "pluck" => "Plucked",
        "drum" => "Drum",
        "sampler" => "Sampler",
        "midi" => "MIDI Inst.",
        "midi_drum" => "MIDI Drum",
        _ => "None",
    }
}

/// An FX type's display name: slot rows, the Type value and auto bus labels.
pub(crate) fn fx_type_label(slot_type: &str) -> &'static str {
    match slot_type {
        "none" => "None",
        "delay" => "Delay",
        "duck" => "Duck",
        "reverb" => "Reverb",
        "tremolo" => "Tremolo",
        "saturator" => "Saturator",
        "distortion" => "Distortion",
        "bitcrusher" => "Bitcrusher",
        "vibrato" => "Vibrato",
        "chorus" => "Chorus",
        "flanger" => "Flanger",
        "filter_lfo" => "Filter LFO",
        "wah" => "Wah",
        "auto_pan" => "Auto Pan",
        "glitch" => "Glitch",
        "compressor" => "Compressor",
        "eq" => "EQ",
        "vinyl" => "Vinyl",
        _ => "",
    }
}

pub(super) fn compact_route_value(value: &str) -> String {
    value
        .strip_prefix("fx_bus_")
        .map(|suffix| format!("fxb{suffix}"))
        .unwrap_or_else(|| value.into())
}

fn format_fx_param_display(key: &str, value: i32) -> String {
    if key.ends_with(".decay") {
        return format_reverb_decay_seconds(value as f64 / 1000.0);
    }
    if key.ends_with("rateHz") {
        return format_fixed_unit(value as f64 / 100.0, "Hz");
    }
    if key.ends_with("Hz") {
        return format_hz(value);
    }
    if key.ends_with("Db") || key.ends_with("GainDb") || key.ends_with("thresholdDb") {
        return format!("{:+.1}dB", value as f64 / 2.0);
    }
    if key.ends_with("clip") || key.ends_with("q") || key.ends_with("midQ") {
        return format_fixed(value as f64 / 100.0, 2);
    }
    if key.ends_with("depthMs") || key.ends_with("baseMs") {
        return format!("{}ms", format_fixed(value as f64 / 10.0, 1));
    }
    if key.ends_with("drive") {
        return format_fixed(value as f64 / 10.0, 1);
    }
    if key.ends_with("ratio") {
        return format!("{}:1", format_fixed(value as f64 / 2.0, 1));
    }
    if key.ends_with("Pct") {
        return format!("{value}%");
    }
    if key.ends_with("Ms") {
        return format_ms(value);
    }
    value.to_string()
}

fn format_fixed_unit(value: f64, unit: &str) -> String {
    format!("{}{unit}", format_fixed(value, 2))
}

fn format_fixed(value: f64, digits: usize) -> String {
    let text = format!("{value:.digits$}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn format_reverb_decay_seconds(value: f64) -> String {
    let feedback = value.clamp(0.0, 0.995);
    if feedback <= 0.0 {
        return "0.0s".into();
    }
    let comb_samples: usize = REVERB_COMB_LENGTHS.iter().sum();
    let average_delay_seconds = comb_samples as f64 / 4.0 / 44_100.0;
    let seconds = (-3.0 * average_delay_seconds) / feedback.log10();
    format!("{seconds:.1}s")
}

fn format_note_with_midi(note: i32) -> String {
    let note = note.clamp(0, 127);
    format!("{} ({note})", note_name(note))
}

/// Sharp note name with middle C as C4, so MIDI 0 is C-1: `C4`, `F#2`.
fn note_name(note: i32) -> String {
    let name = platform_core::NOTE_SET_ROOTS[(note % 12) as usize];
    let octave = note / 12 - 1;
    format!("{name}{octave}")
}

fn format_pan_position(value: i32) -> String {
    let pos = value.clamp(0, 32);
    let distance = pos - 16;
    if distance == 0 {
        "C".into()
    } else if distance < 0 {
        format!("L{}", distance.abs().min(15))
    } else {
        format!("R{}", distance.min(15))
    }
}
