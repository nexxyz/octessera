use octessera_hal::encoder_gpio::HardwareEvent;
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
        HardwareEvent::EncoderRelease { id } => {
            log(format_args!("source=encoder event=raw_release id={id}"));
        }
    }
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
