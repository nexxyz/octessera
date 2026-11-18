use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const RECORD_FILE: &str = "i2c-fault-lines.log";
const RECORD_LINES: usize = 20;

/// The volatile journal is gone after the power cycle a stuck bus needs, so the SDA and SCL levels
/// at fault entry also go to a short file beside the store. SDA low with SCL high means a Seesaw
/// stopped mid-byte; SCL low means one froze while holding the clock.
pub(super) fn record(parts: &str) {
    let levels = match octessera_hal::i2c_bus::seesaw_bus_line_levels() {
        Ok((sda, scl)) => format!("SDA={} SCL={}", level(sda), level(scl)),
        Err(error) => format!("lines unreadable: {error}"),
    };
    eprintln!("hardware fault I2C {levels}");
    let store_dir = crate::main_paths::default_store_dir();
    let path = store_dir.with_file_name(RECORD_FILE);
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    if let Err(error) = append(&path, &format!("{seconds} {levels} {parts}")) {
        eprintln!("hardware fault I2C record failed: {error}");
    }
}

fn level(high: bool) -> &'static str {
    if high {
        "high"
    } else {
        "low"
    }
}

fn append(path: &Path, line: &str) -> std::io::Result<()> {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    std::fs::write(path, with_line(&existing, line))
}

fn with_line(existing: &str, line: &str) -> String {
    let mut lines: Vec<&str> = existing.lines().collect();
    lines.push(line);
    let keep = lines.len().saturating_sub(RECORD_LINES);
    lines[keep..]
        .iter()
        .map(|line| format!("{line}\n"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_record_keeps_only_the_newest_lines() {
        let mut record = String::new();
        for index in 0..RECORD_LINES + 3 {
            record = with_line(&record, &format!("line {index}"));
        }
        let lines: Vec<&str> = record.lines().collect();
        assert_eq!(lines.len(), RECORD_LINES);
        assert_eq!(lines[0], "line 3");
        assert_eq!(
            lines[RECORD_LINES - 1],
            format!("line {}", RECORD_LINES + 2)
        );
        assert!(record.ends_with('\n'));
    }

    #[test]
    fn the_first_line_starts_the_record() {
        assert_eq!(
            with_line("", "1 SDA=low SCL=low Grid"),
            "1 SDA=low SCL=low Grid\n"
        );
    }
}
