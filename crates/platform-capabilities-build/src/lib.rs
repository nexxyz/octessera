use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub fn platform_capabilities_path(manifest_dir: &Path) -> PathBuf {
    manifest_dir.join("../../resources/platform-capabilities.json")
}

pub fn load_platform_capabilities(manifest_dir: &Path) -> Value {
    let source_path = platform_capabilities_path(manifest_dir);
    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {}", source_path.display(), error));
    serde_json::from_str(&source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {}", source_path.display(), error))
}

pub fn positive_usize(value: &Value, key: &str) -> usize {
    value
        .get(key)
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_else(|| {
            panic!(
                "invalid platform capability '{}': expected positive integer",
                key
            )
        })
}

pub fn positive_u8(value: &Value, key: &str) -> u8 {
    value
        .get(key)
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or_else(|| {
            panic!(
                "invalid platform capability '{}': expected positive u8",
                key
            )
        })
}

pub fn validate_voice_lane_capacities(value: &Value) {
    let synth_lane_capacity = positive_usize(value, "synthVoiceLaneCapacity");
    let sample_lane_capacity = positive_usize(value, "sampleVoiceLaneCapacity");
    for key in ["maxSynthVoices", "maxSynthVoicesPerSlot"] {
        validate_voice_policy(value, key, "synthVoiceLaneCapacity", synth_lane_capacity);
    }
    for key in ["maxSampleVoices", "maxSampleVoicesPerSlot"] {
        validate_voice_policy(value, key, "sampleVoiceLaneCapacity", sample_lane_capacity);
    }
}

fn validate_voice_policy(value: &Value, policy_key: &str, lane_key: &str, lane_capacity: usize) {
    let policy_value = positive_usize(value, policy_key);
    if policy_value > lane_capacity {
        panic!(
            "invalid platform capability '{}': {} exceeds physical lane capacity '{}' ({})",
            policy_key, policy_value, lane_key, lane_capacity
        );
    }
}

pub fn scan_section_counts(value: &Value) -> Vec<usize> {
    let entries = value
        .get("scanSectionCounts")
        .and_then(Value::as_array)
        .filter(|entries| !entries.is_empty())
        .unwrap_or_else(|| {
            panic!("invalid platform capability 'scanSectionCounts': expected non-empty array")
        });
    entries
        .iter()
        .map(|entry| {
            entry
                .as_u64()
                .filter(|value| *value > 0)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or_else(|| panic!("invalid scanSectionCounts entry: {}", entry))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::any::Any;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    use std::{env, fs};

    static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

    struct Fixture {
        root: PathBuf,
        manifest_dir: PathBuf,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn panic_message(panic: &(dyn Any + Send)) -> String {
        if let Some(message) = panic.downcast_ref::<String>() {
            return message.clone();
        }
        if let Some(message) = panic.downcast_ref::<&str>() {
            return (*message).to_string();
        }
        "non-string panic".to_string()
    }

    fn assert_panics_with<T>(action: impl FnOnce() -> T, expected: &str) {
        let panic = match catch_unwind(AssertUnwindSafe(action)) {
            Ok(_) => panic!("expected a panic"),
            Err(panic) => panic,
        };
        let message = panic_message(panic.as_ref());
        assert!(
            message.contains(expected),
            "panic did not contain '{}': {}",
            expected,
            message
        );
    }

    fn fixture(source: &str) -> Fixture {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before the Unix epoch")
            .as_nanos();
        let root = loop {
            let fixture_id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
            let candidate = env::temp_dir().join(format!(
                "octessera-platform-capabilities-build-{}-{}-{}",
                std::process::id(),
                timestamp,
                fixture_id
            ));
            match fs::create_dir(&candidate) {
                Ok(()) => break candidate,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("failed to create {}: {}", candidate.display(), error),
            }
        };
        let fixture = Fixture {
            manifest_dir: root.join("crates/platform-core"),
            root,
        };
        fs::create_dir_all(&fixture.manifest_dir).unwrap();
        fs::create_dir_all(fixture.root.join("resources")).unwrap();
        fs::write(platform_capabilities_path(&fixture.manifest_dir), source).unwrap();
        fixture
    }

    #[test]
    fn loads_the_canonical_capabilities_resource() {
        let fixture = fixture(r#"{"gridWidth":8}"#);
        let value = load_platform_capabilities(&fixture.manifest_dir);
        assert_eq!(value["gridWidth"], 8);
    }

    #[test]
    fn rejects_invalid_json_with_the_source_path() {
        let fixture = fixture("{");
        let source_path = platform_capabilities_path(&fixture.manifest_dir);
        assert_panics_with(
            || load_platform_capabilities(&fixture.manifest_dir),
            &format!("failed to parse {}", source_path.display()),
        );
    }

    #[test]
    fn rejects_invalid_positive_usize_values() {
        for value in [
            json!({}),
            json!({"value": 0}),
            json!({"value": -1}),
            json!({"value": 1.5}),
            json!({"value": "1"}),
        ] {
            assert_panics_with(
                || positive_usize(&value, "value"),
                "invalid platform capability 'value': expected positive integer",
            );
        }
    }

    #[test]
    fn rejects_positive_u8_overflow() {
        assert_panics_with(
            || positive_u8(&json!({"value": 256}), "value"),
            "invalid platform capability 'value': expected positive u8",
        );
    }

    #[test]
    fn accepts_voice_policies_within_physical_lane_capacities() {
        let value = json!({
            "synthVoiceLaneCapacity": 64,
            "sampleVoiceLaneCapacity": 64,
            "maxSynthVoices": 16,
            "maxSynthVoicesPerSlot": 8,
            "maxSampleVoices": 64,
            "maxSampleVoicesPerSlot": 8
        });
        validate_voice_lane_capacities(&value);
    }

    #[test]
    fn rejects_nonpositive_voice_lane_capacity() {
        let value = json!({
            "synthVoiceLaneCapacity": 0,
            "sampleVoiceLaneCapacity": 64,
            "maxSynthVoices": 16,
            "maxSynthVoicesPerSlot": 8,
            "maxSampleVoices": 64,
            "maxSampleVoicesPerSlot": 8
        });
        assert_panics_with(
            || validate_voice_lane_capacities(&value),
            "invalid platform capability 'synthVoiceLaneCapacity': expected positive integer",
        );
    }

    #[test]
    fn rejects_voice_policy_above_its_physical_lane_capacity() {
        for (policy_key, lane_key) in [
            ("maxSynthVoices", "synthVoiceLaneCapacity"),
            ("maxSynthVoicesPerSlot", "synthVoiceLaneCapacity"),
            ("maxSampleVoices", "sampleVoiceLaneCapacity"),
            ("maxSampleVoicesPerSlot", "sampleVoiceLaneCapacity"),
        ] {
            let mut value = json!({
                "synthVoiceLaneCapacity": 64,
                "sampleVoiceLaneCapacity": 64,
                "maxSynthVoices": 16,
                "maxSynthVoicesPerSlot": 8,
                "maxSampleVoices": 16,
                "maxSampleVoicesPerSlot": 8
            });
            value[policy_key] = json!(65);
            assert_panics_with(
                || validate_voice_lane_capacities(&value),
                &format!(
                    "invalid platform capability '{}': 65 exceeds physical lane capacity '{}' (64)",
                    policy_key, lane_key
                ),
            );
        }
    }

    #[test]
    fn extracts_non_empty_positive_scan_sections() {
        assert_eq!(
            scan_section_counts(&json!({"scanSectionCounts": [1, 2, 4, 8]})),
            vec![1, 2, 4, 8]
        );
        for value in [
            json!({}),
            json!({"scanSectionCounts": null}),
            json!({"scanSectionCounts": "1"}),
            json!({"scanSectionCounts": []}),
            json!({"scanSectionCounts": [0]}),
            json!({"scanSectionCounts": [-1]}),
            json!({"scanSectionCounts": [1.5]}),
            json!({"scanSectionCounts": ["1"]}),
        ] {
            assert_panics_with(
                || scan_section_counts(&value),
                if value
                    .get("scanSectionCounts")
                    .and_then(Value::as_array)
                    .is_some_and(|entries| !entries.is_empty())
                {
                    "invalid scanSectionCounts entry"
                } else {
                    "invalid platform capability 'scanSectionCounts': expected non-empty array"
                },
            );
        }
    }
}
