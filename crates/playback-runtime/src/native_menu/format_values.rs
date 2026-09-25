pub(super) fn format_display_value(key: Option<&str>, value: impl ToString) -> String {
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
    if key.starts_with("instruments.") && key.ends_with(".type") && raw == "fm" {
        return "FM".into();
    }
    if key.starts_with("instruments.") && key.ends_with(".type") && raw == "pluck" {
        return "Plucked".into();
    }
    if key.starts_with("instruments.") && key.ends_with(".type") && raw == "drum" {
        return "Drum".into();
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
    if key == "play.xy.smoothingMs" {
        return if raw == "0" {
            "Off".into()
        } else {
            format!("{raw} ms")
        };
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
        return format_fx_param_display(key, raw.parse::<i32>().unwrap_or(0));
    }
    if key.ends_with("Pct") || key.ends_with("Percent") {
        return format!("{}%", raw.parse::<i32>().unwrap_or(0));
    }
    if key.ends_with("Ms") {
        let ms = raw.parse::<i32>().unwrap_or(0);
        if ms.abs() >= 1000 {
            return format!("{:.1}s", ms as f32 / 1000.0);
        }
        return format!("{ms}ms");
    }
    raw
}

fn compact_route_value(value: &str) -> String {
    value
        .strip_prefix("fx_bus_")
        .map(|suffix| format!("fxb{suffix}"))
        .unwrap_or_else(|| value.into())
}

fn format_fx_param_display(key: &str, value: i32) -> String {
    if key.ends_with(".decay") {
        return format_reverb_decay_seconds(value as f64 / 1000.0);
    }
    if key.ends_with("Hz") {
        return format_fixed_unit(value as f64 / 100.0, "Hz");
    }
    if key.ends_with("Db") || key.ends_with("GainDb") || key.ends_with("thresholdDb") {
        return format!("{:+.1}dB", value as f64 / 2.0);
    }
    if key.ends_with("feedback")
        || key.ends_with("threshold")
        || key.ends_with("clip")
        || key.ends_with("q")
        || key.ends_with("midQ")
    {
        return format_fixed(value as f64 / 100.0, 2);
    }
    if key.ends_with("drive") || key.ends_with("depthMs") || key.ends_with("baseMs") {
        return format_fixed(value as f64 / 10.0, 1);
    }
    if key.ends_with("ratio") {
        return format_fixed(value as f64 / 2.0, 1);
    }
    if key.ends_with("Pct") {
        return format!("{value}%");
    }
    if key.ends_with("Ms") {
        if value.abs() >= 1000 {
            return format!("{:.1}s", value as f32 / 1000.0);
        }
        return format!("{value}ms");
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
    let average_delay_seconds = ((1557.0 + 1617.0 + 1491.0 + 1422.0) / 4.0) / 44_100.0;
    let seconds = (-3.0 * average_delay_seconds) / feedback.log10();
    format!("{seconds:.1}s")
}

fn format_note_with_midi(note: i32) -> String {
    let note = note.clamp(0, 127);
    let name = platform_core::NOTE_SET_ROOTS[(note % 12) as usize];
    let octave = note / 12 - 1;
    format!("{name}{octave} ({note})")
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
