use super::RuntimeErrorCode;
use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Display, Formatter};

pub const SETUP_PORTAL_SUFFIX_MAX_CHARS: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSetupPortalPhase {
    Starting,
    PortalReady,
    Finalizing,
    Succeeded,
    Failed,
    TimedOut,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSetupPortalDisposition {
    Accepted,
    AlreadyRunning,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSetupPortalErrorCode {
    OperationFailed,
    Unavailable,
    InvalidPayload,
    Unsupported,
}

impl From<RuntimeSetupPortalErrorCode> for RuntimeErrorCode {
    fn from(code: RuntimeSetupPortalErrorCode) -> Self {
        match code {
            RuntimeSetupPortalErrorCode::OperationFailed => Self::OperationFailed,
            RuntimeSetupPortalErrorCode::Unavailable => Self::Unavailable,
            RuntimeSetupPortalErrorCode::InvalidPayload => Self::InvalidPayload,
            RuntimeSetupPortalErrorCode::Unsupported => Self::Unsupported,
        }
    }
}

impl Display for RuntimeSetupPortalErrorCode {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::OperationFailed => "operation_failed",
            Self::Unavailable => "unavailable",
            Self::InvalidPayload => "invalid_payload",
            Self::Unsupported => "unsupported",
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeSetupPortalStatus {
    pub phase: RuntimeSetupPortalPhase,
    pub disposition: Option<RuntimeSetupPortalDisposition>,
    pub portal_suffix: Option<String>,
    pub reboot_required: bool,
    pub error_code: Option<RuntimeSetupPortalErrorCode>,
}

impl RuntimeSetupPortalStatus {
    pub fn validate(&self) -> Result<(), String> {
        if self.reboot_required {
            return Err("setup portal rebootRequired must be false".into());
        }
        if let Some(suffix) = self.portal_suffix.as_deref() {
            validate_portal_suffix(suffix)?;
        }
        match self.phase {
            RuntimeSetupPortalPhase::Starting => {
                if self.disposition.is_none()
                    || self.portal_suffix.is_some()
                    || self.error_code.is_some()
                {
                    return Err(
                        "starting setup portal status requires disposition and cannot contain suffix or error".into(),
                    );
                }
            }
            RuntimeSetupPortalPhase::PortalReady => {
                if self.disposition.is_some() || self.error_code.is_some() {
                    return Err(
                        "portal_ready setup portal status cannot contain disposition or error"
                            .into(),
                    );
                }
                if self.portal_suffix.is_none() {
                    return Err("portal_ready setup portal status requires portalSuffix".into());
                }
            }
            RuntimeSetupPortalPhase::Finalizing | RuntimeSetupPortalPhase::Succeeded => {
                if self.disposition.is_some()
                    || self.portal_suffix.is_some()
                    || self.error_code.is_some()
                {
                    return Err("setup portal completion status contains an invalid field".into());
                }
            }
            RuntimeSetupPortalPhase::Failed => {
                if self.disposition.is_some() || self.portal_suffix.is_some() {
                    return Err(
                        "failed setup portal status cannot contain disposition or suffix".into(),
                    );
                }
                if !matches!(
                    self.error_code,
                    Some(RuntimeSetupPortalErrorCode::OperationFailed)
                        | Some(RuntimeSetupPortalErrorCode::Unavailable)
                        | Some(RuntimeSetupPortalErrorCode::InvalidPayload)
                ) {
                    return Err("failed setup portal status requires a permitted errorCode".into());
                }
            }
            RuntimeSetupPortalPhase::TimedOut => {
                if self.disposition.is_some() || self.portal_suffix.is_some() {
                    return Err(
                        "timed_out setup portal status cannot contain disposition or suffix".into(),
                    );
                }
                if !matches!(
                    self.error_code,
                    Some(RuntimeSetupPortalErrorCode::Unavailable)
                ) {
                    return Err(
                        "timed_out setup portal status requires unavailable errorCode".into(),
                    );
                }
            }
            RuntimeSetupPortalPhase::Unsupported => {
                if self.disposition.is_some() || self.portal_suffix.is_some() {
                    return Err(
                        "unsupported setup portal status cannot contain disposition or suffix"
                            .into(),
                    );
                }
                if !matches!(
                    self.error_code,
                    Some(RuntimeSetupPortalErrorCode::Unsupported)
                ) {
                    return Err(
                        "unsupported setup portal status requires unsupported errorCode".into(),
                    );
                }
            }
        }
        Ok(())
    }
}

fn validate_portal_suffix(value: &str) -> Result<(), String> {
    if value.len() != SETUP_PORTAL_SUFFIX_MAX_CHARS
        || !value
            .bytes()
            .all(|character| character.is_ascii_digit() || matches!(character, b'a'..=b'f'))
    {
        return Err(
            "setup portal portalSuffix must be four lowercase hexadecimal characters".into(),
        );
    }
    Ok(())
}

impl Serialize for RuntimeSetupPortalStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.validate().map_err(serde::ser::Error::custom)?;
        let mut state = serializer.serialize_struct("RuntimeSetupPortalStatus", 5)?;
        state.serialize_field("phase", &self.phase)?;
        if let Some(disposition) = &self.disposition {
            state.serialize_field("disposition", disposition)?;
        }
        if let Some(portal_suffix) = &self.portal_suffix {
            state.serialize_field("portalSuffix", portal_suffix)?;
        }
        state.serialize_field("rebootRequired", &false)?;
        if let Some(error_code) = &self.error_code {
            state.serialize_field("errorCode", error_code)?;
        }
        state.end()
    }
}

impl<'de> Deserialize<'de> for RuntimeSetupPortalStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_struct(
            "RuntimeSetupPortalStatus",
            &[
                "phase",
                "disposition",
                "portalSuffix",
                "rebootRequired",
                "errorCode",
            ],
            RuntimeSetupPortalStatusVisitor,
        )
    }
}

struct RuntimeSetupPortalStatusVisitor;

impl<'de> Visitor<'de> for RuntimeSetupPortalStatusVisitor {
    type Value = RuntimeSetupPortalStatus;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a typed setup portal status")
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut phase = None;
        let mut disposition = None;
        let mut portal_suffix = None;
        let mut reboot_required = None;
        let mut error_code = None;
        while let Some(field) = map.next_key::<RuntimeSetupPortalStatusField>()? {
            match field {
                RuntimeSetupPortalStatusField::Phase => {
                    if phase.is_some() {
                        return Err(serde::de::Error::duplicate_field("phase"));
                    }
                    phase = Some(map.next_value()?);
                }
                RuntimeSetupPortalStatusField::Disposition => {
                    if disposition.is_some() {
                        return Err(serde::de::Error::duplicate_field("disposition"));
                    }
                    disposition = Some(map.next_value()?);
                }
                RuntimeSetupPortalStatusField::PortalSuffix => {
                    if portal_suffix.is_some() {
                        return Err(serde::de::Error::duplicate_field("portalSuffix"));
                    }
                    portal_suffix = Some(map.next_value()?);
                }
                RuntimeSetupPortalStatusField::RebootRequired => {
                    if reboot_required.is_some() {
                        return Err(serde::de::Error::duplicate_field("rebootRequired"));
                    }
                    reboot_required = Some(map.next_value()?);
                }
                RuntimeSetupPortalStatusField::ErrorCode => {
                    if error_code.is_some() {
                        return Err(serde::de::Error::duplicate_field("errorCode"));
                    }
                    error_code = Some(map.next_value()?);
                }
            }
        }
        let status = RuntimeSetupPortalStatus {
            phase: phase.ok_or_else(|| serde::de::Error::missing_field("phase"))?,
            disposition,
            portal_suffix,
            reboot_required: reboot_required
                .ok_or_else(|| serde::de::Error::missing_field("rebootRequired"))?,
            error_code,
        };
        status.validate().map_err(serde::de::Error::custom)?;
        Ok(status)
    }
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum RuntimeSetupPortalStatusField {
    Phase,
    Disposition,
    PortalSuffix,
    RebootRequired,
    ErrorCode,
}
