use super::super::{DeviceInput, NativeOledMode, NativeRunner};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) struct WakeTraceContext {
    input: String,
    mode: &'static str,
    splash: String,
}

impl WakeTraceContext {
    pub(super) fn capture(runner: &NativeRunner, input: &DeviceInput) -> Option<Self> {
        wake_trace_enabled().then(|| Self {
            input: device_input_trace_summary(input),
            mode: oled_mode_trace_name(&runner.display.oled_mode),
            splash: runner.display.oled_splash_text.clone(),
        })
    }
}

pub(super) fn trace_device_input_wake(
    context: Option<&WakeTraceContext>,
    woke_display: bool,
    consumed: bool,
    outcome: &str,
) {
    let Some(context) = context else {
        return;
    };
    eprintln!(
        "wake_trace ts_ms={} source=runtime event=wake_decision mode={} splash={} woke_display={woke_display} consumed={consumed} outcome={outcome} {}",
        wake_trace_timestamp_ms(),
        context.mode,
        context.splash,
        context.input
    );
}

fn device_input_trace_summary(input: &DeviceInput) -> String {
    match input {
        DeviceInput::EncoderTurn { delta, id } => {
            format!("type=encoder_turn id={} delta={delta}", id_trace_value(id))
        }
        DeviceInput::EncoderPress { id } => {
            format!("type=encoder_press id={}", id_trace_value(id))
        }
        DeviceInput::ButtonA { pressed } => button_trace_summary("button_a", *pressed),
        DeviceInput::ButtonS { pressed } => button_trace_summary("button_s", *pressed),
        DeviceInput::ButtonShift { pressed } => button_trace_summary("button_shift", *pressed),
        DeviceInput::ButtonFn { pressed } => button_trace_summary("button_fn", *pressed),
        DeviceInput::ButtonCombinedModifier { pressed } => {
            button_trace_summary("button_combined_modifier", *pressed)
        }
        DeviceInput::GridPress { x, y } => format!("type=grid_press x={x} y={y}"),
        DeviceInput::GridRelease { x, y } => format!("type=grid_release x={x} y={y}"),
        DeviceInput::BehaviorAction(action) => {
            format!("type=behavior_action action_type={}", action.action_type)
        }
        DeviceInput::Other => "type=other".to_string(),
    }
}

fn button_trace_summary(input_type: &str, pressed: Option<bool>) -> String {
    format!("type={input_type} pressed={}", pressed_trace_value(pressed))
}

fn id_trace_value(id: &Option<String>) -> &str {
    id.as_deref().unwrap_or("default")
}

fn pressed_trace_value(pressed: Option<bool>) -> &'static str {
    match pressed {
        Some(true) => "true",
        Some(false) => "false",
        None => "default",
    }
}

fn oled_mode_trace_name(mode: &NativeOledMode) -> &'static str {
    match mode {
        NativeOledMode::Normal => "normal",
        NativeOledMode::Splash => "splash",
        NativeOledMode::Off => "off",
    }
}

fn wake_trace_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("OCTESSERA_WAKE_TRACE")
            .is_ok_and(|value| !matches!(value.as_str(), "" | "0" | "false" | "off"))
    })
}

fn wake_trace_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}
