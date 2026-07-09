use octessera_hal::encoder_gpio::HardwareEvent;
use playback_runtime::HostMessage;
use serde_json::Value;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn log_trellis_event(x: usize, y: usize, pressed: bool) {
    log(format_args!(
        "source=trellis event=grid {} x={x} y={y}",
        press_state(pressed)
    ));
}

pub(crate) fn log_neokey_transition(key: u8, pressed: bool) {
    log(format_args!(
        "source=neokey event=button key={key} {}",
        press_state(pressed)
    ));
}

pub(crate) fn log_encoder_event(event: HardwareEvent) {
    match event {
        HardwareEvent::EncoderTurn { id, delta } => {
            log(format_args!(
                "source=encoder event=raw_turn id={id} delta={delta}"
            ));
        }
        HardwareEvent::EncoderPress { id } => {
            log(format_args!("source=encoder event=raw_press id={id}"));
        }
    }
}

pub(crate) fn log_host_dispatch(message: &HostMessage) {
    if !enabled() {
        return;
    }
    if let HostMessage::DeviceInput { input, .. } = message {
        log(format_args!(
            "source=runtime event=dispatch {}",
            input_summary(input)
        ));
    }
}

fn input_summary(input: &Value) -> String {
    let input_type = input
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let mut fields = vec![format!("type={input_type}")];
    for key in ["id", "x", "y", "pressed", "delta"] {
        if let Some(value) = input.get(key) {
            fields.push(format!("{key}={}", compact_json(value)));
        }
    }
    fields.join(" ")
}

fn compact_json(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}

fn press_state(pressed: bool) -> &'static str {
    if pressed {
        "pressed=true"
    } else {
        "pressed=false"
    }
}

fn log(args: std::fmt::Arguments<'_>) {
    if !enabled() {
        return;
    }
    eprintln!("wake_trace ts_ms={} {args}", timestamp_ms());
}

fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("OCTESSERA_WAKE_TRACE")
            .is_ok_and(|value| !matches!(value.as_str(), "" | "0" | "false" | "off"))
    })
}

fn timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}
