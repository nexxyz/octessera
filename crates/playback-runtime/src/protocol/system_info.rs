use super::RuntimeErrorCode;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSystemInfo {
    pub os: String,
    #[serde(rename = "osVersion")]
    pub os_version: String,
    #[serde(rename = "octesseraVersion")]
    pub octessera_version: String,
    #[serde(rename = "primaryIp")]
    pub primary_ip: Option<String>,
    #[serde(rename = "primaryMac")]
    pub primary_mac: Option<String>,
    pub hostname: String,
    #[serde(rename = "boardProfile")]
    pub board_profile: String,
}

impl RuntimeSystemInfo {
    pub fn sanitized(self) -> Self {
        Self {
            os: sanitize_system_info_text(&self.os),
            os_version: sanitize_system_info_text(&self.os_version),
            octessera_version: sanitize_system_info_text(&self.octessera_version),
            primary_ip: self.primary_ip.as_deref().map(sanitize_system_info_text),
            primary_mac: self.primary_mac.as_deref().map(sanitize_system_info_text),
            hostname: sanitize_system_info_text(&self.hostname),
            board_profile: sanitize_system_info_text(&self.board_profile),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSystemInfoError {
    pub code: RuntimeErrorCode,
    pub message: String,
}

impl RuntimeSystemInfoError {
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            code: RuntimeErrorCode::Unavailable,
            message: sanitize_system_info_text(&message.into()),
        }
    }
}

fn sanitize_system_info_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(96)
        .collect()
}
