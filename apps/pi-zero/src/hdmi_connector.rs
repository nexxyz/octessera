use crate::audio_route::RouteOpenError;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HdmiConnectorState {
    Connected,
    Disconnected,
}

pub(crate) struct HdmiConnectorProbe {
    status_path: PathBuf,
    edid_path: PathBuf,
}

impl HdmiConnectorProbe {
    pub(crate) fn fixed() -> Self {
        let root = std::env::var_os("OCTESSERA_DRM_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/sys/class/drm"));
        Self::from_root(root)
    }

    pub(crate) fn from_root(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            status_path: root.join(fixed_connector_name()).join("status"),
            edid_path: root.join(fixed_connector_name()).join("edid"),
        }
    }

    pub(crate) fn state(&self) -> Result<HdmiConnectorState, RouteOpenError> {
        if !self.status_path.is_file() || !self.edid_path.is_file() {
            return Err(RouteOpenError::Fault(format!(
                "expected HDMI connector paths are missing: {} and {}",
                self.status_path.display(),
                self.edid_path.display()
            )));
        }
        let status = std::fs::read_to_string(&self.status_path).map_err(|error| {
            RouteOpenError::Fault(format!("failed to read HDMI connector status: {error}"))
        })?;
        if !status.trim().eq_ignore_ascii_case("connected") {
            return Ok(HdmiConnectorState::Disconnected);
        }
        let edid = std::fs::read(&self.edid_path).map_err(|error| {
            RouteOpenError::Fault(format!("failed to read HDMI connector EDID: {error}"))
        })?;
        if edid.is_empty() {
            return Ok(HdmiConnectorState::Disconnected);
        }
        Ok(HdmiConnectorState::Connected)
    }

    pub(crate) fn require_connected(&self) -> Result<(), RouteOpenError> {
        match self.state()? {
            HdmiConnectorState::Connected => Ok(()),
            HdmiConnectorState::Disconnected => Err(RouteOpenError::Disconnected),
        }
    }
}

fn fixed_connector_name() -> &'static str {
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    {
        "card0-HDMI-A-1"
    }
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    {
        "card0-HDMI-A-1"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str, connector_name: &str, status: &str, edid: &[u8]) -> HdmiConnectorProbe {
        let root = std::env::temp_dir().join(format!("octessera-hdmi-{name}"));
        let connector = root.join(connector_name);
        std::fs::create_dir_all(&connector).unwrap();
        std::fs::write(connector.join("status"), status).unwrap();
        std::fs::write(connector.join("edid"), edid).unwrap();
        HdmiConnectorProbe::from_root(root)
    }

    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    #[test]
    fn raspberry_profile_pins_card0_connector() {
        assert_eq!(fixed_connector_name(), "card0-HDMI-A-1");
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    #[test]
    fn orange_profile_pins_card0_connector() {
        assert_eq!(fixed_connector_name(), "card0-HDMI-A-1");
    }

    #[test]
    fn connector_state_distinguishes_connected_disconnected_and_empty_edid() {
        assert_eq!(
            fixture("connected", "card0-HDMI-A-1", "connected\n", &[1])
                .state()
                .unwrap(),
            HdmiConnectorState::Connected
        );
        assert_eq!(
            fixture("disconnected", "card0-HDMI-A-1", "disconnected\n", &[1],)
                .state()
                .unwrap(),
            HdmiConnectorState::Disconnected
        );
        assert_eq!(
            fixture("empty-edid", "card0-HDMI-A-1", "connected\n", &[])
                .state()
                .unwrap(),
            HdmiConnectorState::Disconnected
        );
    }

    #[test]
    fn missing_pinned_connector_paths_are_faulted() {
        let root = std::env::temp_dir().join("octessera-hdmi-missing");
        let error = HdmiConnectorProbe::from_root(root).state().unwrap_err();
        assert!(matches!(error, RouteOpenError::Fault(_)));
    }

    #[test]
    fn legacy_card1_fixture_does_not_fallback() {
        let root = std::env::temp_dir().join("octessera-hdmi-legacy-card1");
        let connector = root.join("card1-HDMI-A-1");
        std::fs::create_dir_all(&connector).unwrap();
        std::fs::write(connector.join("status"), "connected\n").unwrap();
        std::fs::write(connector.join("edid"), [1]).unwrap();

        let error = HdmiConnectorProbe::from_root(root).state().unwrap_err();
        assert!(matches!(error, RouteOpenError::Fault(_)));
        assert!(error.to_string().contains("card0-HDMI-A-1"));
    }
}
