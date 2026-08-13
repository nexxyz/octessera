use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Deserializer, Serializer};

pub(crate) mod base64_bytes {
    use super::*;

    pub(crate) fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[cfg(test)]
        super::record_base64_encode();
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        STANDARD.decode(value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static BASE64_ENCODE_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
fn record_base64_encode() {
    BASE64_ENCODE_COUNT.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
pub(crate) fn reset_base64_encode_count() {
    BASE64_ENCODE_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn base64_encode_count() -> usize {
    BASE64_ENCODE_COUNT.with(Cell::get)
}
