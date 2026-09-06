use crate::audio::AudioSink;
use playback_runtime::AudioOutputSet;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RouteOpenError {
    #[allow(dead_code)]
    Absent,
    Disconnected,
    Busy,
    Unsupported(String),
    Fault(String),
}

impl RouteOpenError {
    pub(crate) fn status(&self) -> AudioRouteStatus {
        match self {
            Self::Absent | Self::Disconnected => AudioRouteStatus::Waiting,
            Self::Busy | Self::Unsupported(_) | Self::Fault(_) => AudioRouteStatus::Faulted,
        }
    }

    pub(crate) fn is_waiting(&self) -> bool {
        matches!(self, Self::Absent | Self::Disconnected)
    }
}

impl std::fmt::Display for RouteOpenError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Absent => formatter.write_str("audio route is absent"),
            Self::Disconnected => formatter.write_str("audio route is disconnected"),
            Self::Busy => formatter.write_str("audio route is busy"),
            Self::Unsupported(message) | Self::Fault(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for RouteOpenError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AudioRouteStatus {
    Active,
    Waiting,
    Faulted,
}

pub(crate) type AudioRouteRegistry = Arc<Mutex<BTreeMap<AudioSink, AudioRouteStatus>>>;

pub(crate) fn new_registry(outputs: AudioOutputSet) -> AudioRouteRegistry {
    let mut routes = BTreeMap::new();
    for sink in AudioSink::selected(outputs) {
        routes.insert(sink, AudioRouteStatus::Waiting);
    }
    Arc::new(Mutex::new(routes))
}

pub(crate) fn set_status(registry: &AudioRouteRegistry, sink: AudioSink, status: AudioRouteStatus) {
    if let Ok(mut routes) = registry.lock() {
        routes.insert(sink, status);
    }
}

pub(crate) fn status(registry: &AudioRouteRegistry, sink: AudioSink) -> AudioRouteStatus {
    registry
        .lock()
        .ok()
        .and_then(|routes| routes.get(&sink).copied())
        .unwrap_or(AudioRouteStatus::Faulted)
}

#[cfg(any(test, not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) fn readiness(
    _outputs: AudioOutputSet,
    registry: &AudioRouteRegistry,
) -> Result<(), String> {
    if status(registry, AudioSink::Jack) != AudioRouteStatus::Active {
        return Err("selected Jack audio route is not active".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_output_sets_select_only_enabled_routes() {
        for flags in [
            (true, false, false),
            (false, true, false),
            (false, false, true),
            (true, true, false),
            (true, false, true),
            (false, true, true),
            (true, true, true),
        ] {
            let outputs = AudioOutputSet::from_flags(flags.0, flags.1, flags.2).unwrap();
            let selected = AudioSink::selected(outputs);
            assert_eq!(selected.contains(&AudioSink::Jack), flags.0);
            assert_eq!(selected.contains(&AudioSink::Usb), flags.1);
            assert_eq!(selected.contains(&AudioSink::Hdmi), flags.2);
        }
    }

    #[test]
    fn missing_routes_wait_but_stream_and_format_failures_fault() {
        assert_eq!(RouteOpenError::Absent.status(), AudioRouteStatus::Waiting);
        assert_eq!(RouteOpenError::Busy.status(), AudioRouteStatus::Faulted);
        assert_eq!(
            RouteOpenError::Unsupported("format".into()).status(),
            AudioRouteStatus::Faulted
        );
    }

    #[test]
    fn readiness_requires_active_jack_and_accepts_waiting_optional_routes() {
        let outputs = AudioOutputSet::from_flags(true, true, true).unwrap();
        let registry = new_registry(outputs);
        set_status(&registry, AudioSink::Jack, AudioRouteStatus::Active);
        assert!(readiness(outputs, &registry).is_ok());
        for sink in [AudioSink::Usb, AudioSink::Hdmi] {
            set_status(&registry, sink, AudioRouteStatus::Waiting);
            assert!(readiness(outputs, &registry).is_ok());
            assert_eq!(status(&registry, sink), AudioRouteStatus::Waiting);
        }
    }

    #[test]
    fn selected_optional_route_fault_does_not_block_readiness() {
        let outputs = AudioOutputSet::from_flags(true, true, true).unwrap();
        let registry = new_registry(outputs);
        set_status(&registry, AudioSink::Jack, AudioRouteStatus::Active);
        for sink in [AudioSink::Usb, AudioSink::Hdmi] {
            set_status(&registry, sink, AudioRouteStatus::Faulted);
            assert!(readiness(outputs, &registry).is_ok());
            set_status(&registry, sink, AudioRouteStatus::Waiting);
        }
    }

    #[test]
    fn jack_is_required_even_when_not_selected_as_a_user_output() {
        let outputs = AudioOutputSet::from_flags(false, true, false).unwrap();
        let registry = new_registry(outputs);

        assert_eq!(
            readiness(outputs, &registry).unwrap_err(),
            "selected Jack audio route is not active"
        );
    }

    #[test]
    fn jack_fault_is_fatal() {
        let outputs = AudioOutputSet::from_flags(true, true, true).unwrap();
        let registry = new_registry(outputs);
        set_status(&registry, AudioSink::Jack, AudioRouteStatus::Faulted);

        assert!(readiness(outputs, &registry).is_err());
    }

    #[test]
    fn unselected_route_faults_are_ignored() {
        let outputs = AudioOutputSet::jack();
        let registry = new_registry(outputs);

        set_status(&registry, AudioSink::Jack, AudioRouteStatus::Active);
        set_status(&registry, AudioSink::Usb, AudioRouteStatus::Faulted);
        set_status(&registry, AudioSink::Hdmi, AudioRouteStatus::Faulted);

        assert!(readiness(outputs, &registry).is_ok());
    }

    #[test]
    fn disconnected_optional_route_waits_without_blocking_readiness() {
        let outputs = AudioOutputSet::from_flags(true, true, false).unwrap();
        let registry = new_registry(outputs);

        assert_eq!(
            AudioSink::selected(outputs),
            vec![AudioSink::Jack, AudioSink::Usb]
        );
        assert_eq!(
            registry.lock().unwrap().keys().copied().collect::<Vec<_>>(),
            vec![AudioSink::Jack, AudioSink::Usb]
        );
        set_status(&registry, AudioSink::Jack, AudioRouteStatus::Active);
        set_status(
            &registry,
            AudioSink::Usb,
            RouteOpenError::Disconnected.status(),
        );
        assert!(readiness(outputs, &registry).is_ok());
        assert_eq!(status(&registry, AudioSink::Usb), AudioRouteStatus::Waiting);
        assert!(!registry.lock().unwrap().contains_key(&AudioSink::Hdmi));
    }
}
