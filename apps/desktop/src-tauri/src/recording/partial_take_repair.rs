use std::path::{Path, PathBuf};

/// Saves Audio+Video takes an abrupt exit left as `.partial.avi`, as the boards do at startup.
/// They are listed before any take can start; the repair itself runs on its own thread.
pub(crate) fn spawn_partial_take_repair(dir: &Path) {
    let partials = media_recording::partial_avi_paths(dir);
    if partials.is_empty() {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("octessera-avi-repair".into())
        .spawn(move || repair_partial_takes(partials));
    if let Err(error) = spawned {
        eprintln!("interrupted recording repair could not start: {error}");
    }
}

fn repair_partial_takes(partials: Vec<PathBuf>) {
    for partial in partials {
        match media_recording::repair_partial_avi(&partial) {
            Ok(outcome) => eprintln!(
                "interrupted recording saved: path={} frames={}",
                outcome.path.display(),
                outcome.frames_written
            ),
            Err(error) => eprintln!("interrupted recording left as is: {error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::repair_partial_takes;
    use crate::recording::DesktopRecording;
    use std::fs;
    use std::time::Duration;

    #[test]
    fn a_partial_audio_video_take_is_saved_as_incomplete() {
        let root = std::env::temp_dir().join(format!(
            "octessera-desktop-avi-repair-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let screen = root.join("screen-recordings");
        let recording = DesktopRecording::with_screen_directory(root.join("audio"), screen.clone());
        recording.start_audio_oled(1).unwrap();
        std::thread::sleep(Duration::from_millis(50));
        let crashed = root.join("crashed");
        fs::create_dir_all(&crashed).unwrap();
        let active = media_recording::partial_avi_paths(&screen);
        assert_eq!(active.len(), 1);
        fs::copy(&active[0], crashed.join("interrupted.partial.avi")).unwrap();
        recording.stop_audio().unwrap();

        repair_partial_takes(media_recording::partial_avi_paths(&crashed));

        let names = fs::read_dir(&crashed)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(names, ["interrupted.incomplete.avi"]);
        let _ = fs::remove_dir_all(root);
    }
}
