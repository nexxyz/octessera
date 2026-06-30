use realtime_engine::synth::VoiceStealingMode;

pub fn parse_voice_stealing_mode(raw: &str) -> VoiceStealingMode {
    match raw {
        "none" | "off" => VoiceStealingMode::None,
        "fixed12" => VoiceStealingMode::Fixed12,
        "fixed16" => VoiceStealingMode::Fixed16,
        "auto-soft" | "lenient" => VoiceStealingMode::AutoSoft,
        "auto-hard" | "aggressive" => VoiceStealingMode::AutoHard,
        _ => VoiceStealingMode::AutoBalanced,
    }
}
