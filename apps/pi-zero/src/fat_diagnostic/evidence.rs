use super::model::{EvidenceCheck, EvidenceReport};
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

pub(crate) struct EvidenceWriter {
    root: PathBuf,
}

impl EvidenceWriter {
    pub(crate) fn new(root: &Path) -> Result<Self, String> {
        match fs::symlink_metadata(root) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!("evidence path is a symlink: {}", root.display()))
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(format!(
                    "evidence path is not a directory: {}",
                    root.display()
                ))
            }
            Ok(_) => {
                return Err(format!(
                    "evidence path must be newly created and empty: {}",
                    root.display()
                ))
            }
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                return Err(format!("cannot inspect evidence path: {error}"))
            }
            Err(_) => {}
        }
        fs::create_dir(root)
            .map_err(|error| format!("cannot create evidence directory: {error}"))?;
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    pub(crate) fn write_artifact(&self, name: &str, content: &str) -> Result<String, String> {
        if !safe_artifact_name(name) {
            return Err(format!("unsafe evidence artifact name: {name}"));
        }
        self.write_new_file(name, sanitize_text(content).as_bytes())?;
        Ok(name.to_string())
    }

    pub(crate) fn write_report(&self, report: &EvidenceReport) -> Result<(), String> {
        let mut value = serde_json::to_value(report)
            .map_err(|error| format!("cannot serialize diagnostic evidence: {error}"))?;
        sanitize_json_strings(&mut value);
        let payload = serde_json::to_vec_pretty(&value)
            .map_err(|error| format!("cannot serialize diagnostic evidence: {error}"))?;
        self.write_new_file("fat-diagnostic.json", &payload)
    }

    fn write_new_file(&self, name: &str, content: &[u8]) -> Result<(), String> {
        let path = self.root.join(name);
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.custom_flags(libc::O_NOFOLLOW);
        let mut file = options.open(&path).map_err(|error| {
            format!(
                "cannot create evidence artifact {}: {error}",
                path.display()
            )
        })?;
        file.write_all(content)
            .map_err(|error| format!("cannot write evidence artifact {}: {error}", path.display()))
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
}

fn sanitize_json_strings(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(text) => *text = sanitize_text(text),
        serde_json::Value::Array(values) => {
            for value in values {
                sanitize_json_strings(value);
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values_mut() {
                sanitize_json_strings(value);
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
}

pub(crate) fn sanitize_text(text: &str) -> String {
    let mut in_private_key = false;
    text.lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            let begins_private_key = lower.contains("begin ") && lower.contains("private key");
            let ends_private_key = lower.contains("end ") && lower.contains("private key");
            let sanitized = if in_private_key || begins_private_key {
                "[REDACTED PRIVATE KEY]".into()
            } else {
                sanitize_line(line)
            };
            if begins_private_key {
                in_private_key = true;
            }
            if ends_private_key {
                in_private_key = false;
            }
            sanitized
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn sanitize_line(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    if lower.contains("begin private key")
        || lower.contains("password")
        || lower.contains("passphrase")
        || lower.contains("wpa_psk")
        || lower.contains("ssid")
        || lower.contains("transfer-code")
        || lower.contains("transfercode")
        || lower.contains("request_token")
        || lower.contains("authorization:")
        || lower.contains("bearer ")
        || lower.contains("cookie:")
        || lower.contains("secret")
        || lower.contains("token=")
        || lower.contains("token:")
        || lower.contains("credential")
        || lower.contains("api_key")
        || lower.contains("access_key")
    {
        return "[REDACTED SENSITIVE EVIDENCE LINE]".into();
    }
    line.replace('\0', "\\0")
}

fn safe_artifact_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
}

pub(crate) fn format_check_log(checks: &[EvidenceCheck]) -> String {
    checks
        .iter()
        .map(|check| {
            format!(
                "{}\t{:?}\t{}ms\t{}\t{}",
                check.id, check.status, check.elapsed_ms, check.message, check.artifact
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::{safe_artifact_name, sanitize_text};

    #[test]
    fn evidence_redacts_secrets_without_redacting_profile_facts() {
        let sanitized = sanitize_text(
            "OCTESSERA_BOARD_PROFILE_ID=orange-pi-zero-2w\nssid=studio\nrequest_token=abc\nrequestToken=abc2\ntransferCode=def\ntoken=ghi\npassword=pwd\nsecret=shh\nAuthorization: Bearer jkl\n",
        );
        assert!(sanitized.contains("orange-pi-zero-2w"));
        assert!(!sanitized.contains("studio"));
        assert!(!sanitized.contains("abc"));
        assert!(!sanitized.contains("abc2"));
        assert!(!sanitized.contains("def"));
        assert!(!sanitized.contains("ghi"));
        assert!(!sanitized.contains("pwd"));
        assert!(!sanitized.contains("shh"));
        assert!(!sanitized.contains("jkl"));
    }

    #[test]
    fn evidence_redacts_private_key_headers_and_body() {
        let sanitized = sanitize_text(
            "-----BEGIN OPENSSH PRIVATE KEY-----\nprivate-key-body\n-----END OPENSSH PRIVATE KEY-----",
        );
        assert!(!sanitized.contains("private-key-body"));
        assert!(sanitized.contains("REDACTED PRIVATE KEY"));
    }

    #[test]
    fn report_string_values_are_sanitized_before_serialization() {
        let mut value = serde_json::json!({
            "message": "token=do-not-share",
            "nested": ["Authorization: Bearer do-not-share"]
        });
        super::sanitize_json_strings(&mut value);
        let serialized = value.to_string();
        assert!(!serialized.contains("do-not-share"));
        assert!(serialized.contains("REDACTED SENSITIVE EVIDENCE LINE"));
    }

    #[test]
    fn artifact_names_cannot_escape_the_evidence_directory() {
        assert!(safe_artifact_name("service.txt"));
        assert!(!safe_artifact_name("../service.txt"));
        assert!(!safe_artifact_name("nested/service.txt"));
        assert!(!safe_artifact_name(""));
    }

    #[test]
    fn evidence_directory_must_be_new_and_artifacts_are_create_new() {
        let root = std::env::temp_dir().join(format!(
            "octessera-fat-evidence-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let writer = super::EvidenceWriter::new(&root).unwrap();
        writer.write_artifact("status.txt", "first").unwrap();
        assert!(writer.write_artifact("status.txt", "second").is_err());
        assert!(super::EvidenceWriter::new(&root).is_err());
        assert_eq!(
            std::fs::read_to_string(root.join("status.txt")).unwrap(),
            "first"
        );
        let file_root = root.with_extension("file");
        std::fs::write(&file_root, "not a directory").unwrap();
        assert!(super::EvidenceWriter::new(&file_root).is_err());
        std::fs::remove_file(file_root).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn evidence_directory_symlinks_are_rejected() {
        use std::os::unix::fs::symlink;

        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let target = std::env::temp_dir().join(format!(
            "octessera-fat-evidence-target-{}-{suffix}",
            std::process::id(),
        ));
        let link = std::env::temp_dir().join(format!(
            "octessera-fat-evidence-link-{}-{suffix}",
            std::process::id(),
        ));
        let _ = std::fs::remove_dir_all(&target);
        let _ = std::fs::remove_file(&link);
        std::fs::create_dir(&target).unwrap();
        symlink(&target, &link).unwrap();
        assert!(super::EvidenceWriter::new(&link).is_err());
        std::fs::remove_file(link).unwrap();
        std::fs::remove_dir(target).unwrap();
    }
}
