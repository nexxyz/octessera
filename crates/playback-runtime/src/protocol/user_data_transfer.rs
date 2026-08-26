use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const USER_DATA_TRANSFER_CODE_LENGTH: usize = 10;
pub const USER_DATA_TRANSFER_CODE_ALPHABET: &[u8] =
    b"23456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeUserDataTransferPhase {
    Ready,
    Closed,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeUserDataTransferStatus {
    pub phase: RuntimeUserDataTransferPhase,
    pub url: Option<String>,
    pub code: Option<String>,
    pub expires_in_seconds: Option<u16>,
}

impl RuntimeUserDataTransferStatus {
    pub fn validate(&self) -> Result<(), String> {
        match self.phase {
            RuntimeUserDataTransferPhase::Ready => {
                let url = self
                    .url
                    .as_deref()
                    .ok_or_else(|| "ready user-data transfer status requires url".to_string())?;
                validate_http_url(url)?;
                let code = self
                    .code
                    .as_deref()
                    .ok_or_else(|| "ready user-data transfer status requires code".to_string())?;
                validate_transfer_code(code)?;
                let expires = self.expires_in_seconds.ok_or_else(|| {
                    "ready user-data transfer status requires expiresInSeconds".to_string()
                })?;
                if !(1..=900).contains(&expires) {
                    return Err("user-data transfer expiresInSeconds must be 1..=900".into());
                }
            }
            RuntimeUserDataTransferPhase::Closed | RuntimeUserDataTransferPhase::Unsupported => {
                if self.url.is_some() || self.code.is_some() || self.expires_in_seconds.is_some() {
                    return Err(
                        "closed or unsupported user-data transfer status cannot contain transfer fields"
                            .into(),
                    );
                }
            }
        }
        Ok(())
    }
}

impl Serialize for RuntimeUserDataTransferStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.validate().map_err(serde::ser::Error::custom)?;
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Wire<'a> {
            phase: &'a RuntimeUserDataTransferPhase,
            #[serde(skip_serializing_if = "Option::is_none")]
            url: &'a Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            code: &'a Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            expires_in_seconds: &'a Option<u16>,
        }
        Wire {
            phase: &self.phase,
            url: &self.url,
            code: &self.code,
            expires_in_seconds: &self.expires_in_seconds,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RuntimeUserDataTransferStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        #[serde(rename_all = "camelCase")]
        struct Wire {
            phase: RuntimeUserDataTransferPhase,
            #[serde(default)]
            url: Option<String>,
            #[serde(default)]
            code: Option<String>,
            #[serde(default)]
            expires_in_seconds: Option<u16>,
        }
        let wire = Wire::deserialize(deserializer)?;
        let status = Self {
            phase: wire.phase,
            url: wire.url,
            code: wire.code,
            expires_in_seconds: wire.expires_in_seconds,
        };
        status.validate().map_err(D::Error::custom)?;
        Ok(status)
    }
}

fn validate_http_url(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value.starts_with("http://")
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err("user-data transfer url must be a valid http URL".into());
    }
    let authority = value[7..].split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty() || authority.contains('@') {
        return Err("user-data transfer url must contain a host".into());
    }
    if authority.starts_with('[') {
        let Some(end) = authority.find(']') else {
            return Err("user-data transfer url must contain a valid host".into());
        };
        if end <= 1 {
            return Err("user-data transfer url must contain a valid host".into());
        }
        let port = &authority[end + 1..];
        if !port.is_empty() && (!port.starts_with(':') || !valid_port(&port[1..])) {
            return Err("user-data transfer url must contain a valid port".into());
        }
    } else {
        if authority.contains(':') {
            let Some((host, port)) = authority.rsplit_once(':') else {
                unreachable!();
            };
            if host.is_empty() || host.contains(':') || port.is_empty() || !valid_port(port) {
                return Err("user-data transfer url must contain a valid port".into());
            }
        } else if authority.is_empty() {
            return Err("user-data transfer url must contain a valid host".into());
        }
    }
    Ok(())
}

fn valid_port(value: &str) -> bool {
    value.parse::<u16>().is_ok_and(|port| port > 0)
}

fn validate_transfer_code(value: &str) -> Result<(), String> {
    if value.len() != USER_DATA_TRANSFER_CODE_LENGTH
        || !value
            .bytes()
            .all(|character| USER_DATA_TRANSFER_CODE_ALPHABET.contains(&character))
    {
        return Err("user-data transfer code is invalid".into());
    }
    Ok(())
}
