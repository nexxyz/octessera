use std::time::{Duration, Instant};

const CHECK_INTERVAL: Duration = Duration::from_secs(1);

/// A device node name and its inode: a gadget rebuilt between two listings
/// gets the same name back, but a new node.
type DeviceNode = (String, u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NodeChange {
    Unchanged,
    AddedOrRemoved,
    /// A node came back under its old name between two listings, so its device was replaced.
    Recreated,
}

/// Notices MIDI devices being plugged in or unplugged by listing the ALSA MIDI
/// device nodes once a second.
#[derive(Default)]
pub(super) struct PortWatch {
    nodes: Option<Vec<DeviceNode>>,
    checked_at: Option<Instant>,
}

impl PortWatch {
    /// Lists the nodes when a check is due and tells how they changed.
    pub(super) fn check(&mut self, now: Instant) -> Option<NodeChange> {
        if self
            .checked_at
            .is_some_and(|at| now.duration_since(at) < CHECK_INTERVAL)
        {
            return None;
        }
        self.checked_at = Some(now);
        Some(self.observe(midi_device_nodes()))
    }

    #[cfg(test)]
    pub(super) fn assume_known(&mut self, nodes: Vec<DeviceNode>) {
        self.nodes = Some(nodes);
        self.checked_at = None;
    }

    fn observe(&mut self, nodes: Vec<DeviceNode>) -> NodeChange {
        let change = match &self.nodes {
            Some(known) if *known != nodes => {
                if known.iter().any(|(name, inode)| {
                    nodes
                        .iter()
                        .any(|(other, other_inode)| other == name && other_inode != inode)
                }) {
                    NodeChange::Recreated
                } else {
                    NodeChange::AddedOrRemoved
                }
            }
            _ => NodeChange::Unchanged,
        };
        self.nodes = Some(nodes);
        change
    }
}

fn midi_device_nodes() -> Vec<DeviceNode> {
    let mut nodes: Vec<DeviceNode> = std::fs::read_dir("/dev/snd")
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().into_string().ok()?;
            name.starts_with("midiC")
                .then(|| (name, node_inode(&entry)))
        })
        .collect();
    nodes.sort_unstable();
    nodes
}

#[cfg(unix)]
fn node_inode(entry: &std::fs::DirEntry) -> u64 {
    std::os::unix::fs::DirEntryExt::ino(entry)
}

#[cfg(not(unix))]
fn node_inode(_entry: &std::fs::DirEntry) -> u64 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_listing_is_the_baseline_and_later_differences_are_changes() {
        let mut watch = PortWatch::default();
        let one = |inode| vec![("midiC1D0".to_string(), inode)];
        assert_eq!(watch.observe(one(7)), NodeChange::Unchanged);
        assert_eq!(watch.observe(one(7)), NodeChange::Unchanged);
        assert_eq!(watch.observe(Vec::new()), NodeChange::AddedOrRemoved);
        assert_eq!(watch.observe(one(7)), NodeChange::AddedOrRemoved);
        assert_eq!(
            watch.observe(vec![("midiC1D0".into(), 7), ("midiC2D0".into(), 9)]),
            NodeChange::AddedOrRemoved
        );
        assert_eq!(watch.observe(one(8)), NodeChange::Recreated);
    }

    #[test]
    fn devices_are_listed_at_most_once_a_second() {
        let mut watch = PortWatch::default();
        let start = Instant::now();
        watch.check(start);
        watch.nodes = Some(vec![("stale".into(), 1)]);
        assert_eq!(watch.check(start + Duration::from_millis(500)), None);
        assert_eq!(watch.nodes, Some(vec![("stale".into(), 1)]));
    }
}
