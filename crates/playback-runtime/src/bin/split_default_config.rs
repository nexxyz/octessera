use playback_runtime::split_system_patch_documents;
use serde_json::Value;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args_os().skip(1);
    let Some(input) = args.next() else {
        eprintln!(
            "usage: split_default_config <generated-mixed-default.json> <empty-build-output-dir>"
        );
        return ExitCode::from(2);
    };
    let Some(output_dir) = args.next() else {
        eprintln!(
            "usage: split_default_config <generated-mixed-default.json> <empty-build-output-dir>"
        );
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!(
            "usage: split_default_config <generated-mixed-default.json> <empty-build-output-dir>"
        );
        return ExitCode::from(2);
    }
    match split_to_directory(Path::new(&input), Path::new(&output_dir)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("split_default_config: {error}");
            ExitCode::FAILURE
        }
    }
}

fn split_to_directory(input: &Path, output_dir: &Path) -> Result<(), String> {
    let bytes = fs::read(input).map_err(|error| format!("cannot read input: {error}"))?;
    let full: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("input is not valid JSON: {error}"))?;
    let documents = split_system_patch_documents(&full)
        .map_err(|error| format!("input config is invalid: {error}"))?;
    let system_bytes = serde_json::to_vec_pretty(&documents.system)
        .map_err(|error| format!("cannot serialize System document: {error}"))?;
    let patch_bytes = serde_json::to_vec_pretty(&documents.patch)
        .map_err(|error| format!("cannot serialize Patch document: {error}"))?;

    let output_exists = validate_output_directory(output_dir)?;
    if !output_exists {
        fs::create_dir(output_dir)
            .map_err(|error| format!("cannot create output directory: {error}"))?;
    }
    write_new(&output_dir.join("system.json"), &system_bytes)?;
    write_new(&output_dir.join("default.patch.json"), &patch_bytes)
}

fn validate_output_directory(output_dir: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(output_dir) {
        Ok(metadata) => {
            if !metadata.file_type().is_dir() {
                return Err("output path exists and is not a directory".into());
            }
            for name in ["system.json", "default.patch.json"] {
                if path_exists(&output_dir.join(name))? {
                    return Err(format!("output file already exists: {name}"));
                }
            }
            if fs::read_dir(output_dir)
                .map_err(|error| format!("cannot inspect output directory: {error}"))?
                .next()
                .is_some()
            {
                return Err("output directory is not empty".into());
            }
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = output_dir
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            if !fs::metadata(parent)
                .map(|metadata| metadata.is_dir())
                .unwrap_or(false)
            {
                return Err("output directory parent must already exist".into());
            }
            Ok(false)
        }
        Err(error) => Err(format!("cannot inspect output path: {error}")),
    }
}

fn path_exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("cannot inspect output file: {error}")),
    }
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("cannot create output file: {error}"))?;
    file.write_all(bytes)
        .map_err(|error| format!("cannot write output file: {error}"))?;
    file.flush()
        .map_err(|error| format!("cannot flush output file: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    const DESKTOP_DEFAULT: &str = include_str!("../../../../config/generated/desktop/default.json");
    const PI_DEFAULT: &str = include_str!("../../../../config/generated/pi/default.json");

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = env::temp_dir().join(format!(
                "split-default-config-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn generated_desktop_and_pi_defaults_write_native_system_and_patch_documents() {
        for (name, source) in [("desktop", DESKTOP_DEFAULT), ("pi", PI_DEFAULT)] {
            let test_dir = TestDirectory::new();
            let input = test_dir.0.join(format!("{name}.json"));
            let output = test_dir.0.join("build-output");
            fs::write(&input, source).unwrap();
            let full: Value = serde_json::from_str(source).unwrap();
            let expected = split_system_patch_documents(&full).unwrap();

            split_to_directory(&input, &output).unwrap();

            let system: Value =
                serde_json::from_slice(&fs::read(output.join("system.json")).unwrap()).unwrap();
            let patch: Value =
                serde_json::from_slice(&fs::read(output.join("default.patch.json")).unwrap())
                    .unwrap();
            assert_eq!(system, expected.system);
            assert_eq!(patch, expected.patch);
        }
    }

    #[test]
    fn invalid_config_and_existing_output_fail_without_overwriting() {
        let test_dir = TestDirectory::new();
        let invalid_input = test_dir.0.join("wrong-kind.json");
        let output = test_dir.0.join("not-created");
        fs::write(
            &invalid_input,
            r#"{"kind":"octessera.patch","schemaVersion":2}"#,
        )
        .unwrap();
        assert!(split_to_directory(&invalid_input, &output).is_err());
        assert!(!output.exists());

        let full: Value = serde_json::from_str(PI_DEFAULT).unwrap();
        let valid_input = test_dir.0.join("valid.json");
        fs::write(&valid_input, serde_json::to_vec(&full).unwrap()).unwrap();
        let existing = test_dir.0.join("existing-output");
        fs::create_dir(&existing).unwrap();
        let sentinel = existing.join("system.json");
        fs::write(&sentinel, b"keep").unwrap();
        assert!(split_to_directory(&valid_input, &existing).is_err());
        assert_eq!(fs::read(sentinel).unwrap(), b"keep");
        assert!(!existing.join("default.patch.json").exists());
    }
}
