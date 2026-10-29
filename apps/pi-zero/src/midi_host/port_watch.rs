use std::time::{Duration, Instant};

const CHECK_INTERVAL: Duration = Duration::from_secs(1);

/// Notices MIDI devices being plugged in or unplugged by listing the ALSA MIDI
/// device nodes once a second.
#[derive(Default)]
pub(super) struct PortWatch {
    nodes: Option<Vec<String>>,
    checked_at: Option<Instant>,
}

impl PortWatch {
    pub(super) fn changed(&mut self, now: Instant) -> bool {
        if self
            .checked_at
            .is_some_and(|at| now.duration_since(at) < CHECK_INTERVAL)
        {
            return false;
        }
        self.checked_at = Some(now);
        self.observe(midi_device_nodes())
    }

    #[cfg(test)]
    pub(super) fn assume_known(&mut self, nodes: Vec<String>) {
        self.nodes = Some(nodes);
        self.checked_at = None;
    }

    fn observe(&mut self, nodes: Vec<String>) -> bool {
        let changed = self.nodes.as_ref().is_some_and(|known| *known != nodes);
        self.nodes = Some(nodes);
        changed
    }
}

fn midi_device_nodes() -> Vec<String> {
    let mut nodes: Vec<String> = std::fs::read_dir("/dev/snd")
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.starts_with("midiC"))
        .collect();
    nodes.sort_unstable();
    nodes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_listing_is_the_baseline_and_later_differences_are_changes() {
        let mut watch = PortWatch::default();
        assert!(!watch.observe(vec!["midiC1D0".into()]));
        assert!(!watch.observe(vec!["midiC1D0".into()]));
        assert!(watch.observe(Vec::new()));
        assert!(watch.observe(vec!["midiC1D0".into()]));
    }

    #[test]
    fn devices_are_listed_at_most_once_a_second() {
        let mut watch = PortWatch::default();
        let start = Instant::now();
        watch.changed(start);
        watch.nodes = Some(vec!["stale".into()]);
        assert!(!watch.changed(start + Duration::from_millis(500)));
        assert_eq!(watch.nodes, Some(vec!["stale".into()]));
    }
}
