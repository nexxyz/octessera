//! Notices a card being mounted in the OLED board's SD slot (SD2), so the runtime loads the
//! patch's card samples that are not loaded yet, and a card the SD helper's pre-mount check
//! repaired, so the runtime says so once.

use playback_runtime::{HostMessage, RuntimeStoreResult};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_secs(1);
const MOUNTINFO_PATH: &str = "/proc/self/mountinfo";
/// Written by the SD helper after a repair that left the card clean: device and disk sequence.
const REPAIR_MARKER_PATH: &str = "/run/octessera-sd-card.repaired";

pub(crate) struct SdCardWatch {
    mount_point: PathBuf,
    mounted: Option<String>,
    repair_marker: PathBuf,
    repair_shown: Option<String>,
    next_poll: Instant,
}

impl SdCardWatch {
    pub(crate) fn new(samples_dir: &Path) -> Self {
        let mount_point = samples_dir.join(crate::sample_browser::SD_CARD_SAMPLE_DIR);
        Self {
            mounted: current_card(&mount_point),
            mount_point,
            repair_marker: REPAIR_MARKER_PATH.into(),
            repair_shown: None,
            next_poll: Instant::now() + POLL_INTERVAL,
        }
    }

    pub(crate) fn poll(&mut self, now: Instant) -> Option<HostMessage> {
        if now < self.next_poll {
            return None;
        }
        self.next_poll = now + POLL_INTERVAL;
        self.observe(current_card(&self.mount_point))
            .or_else(|| self.observe_repair(std::fs::read_to_string(&self.repair_marker).ok()))
    }

    fn observe(&mut self, mounted: Option<String>) -> Option<HostMessage> {
        let arrived = mounted.is_some() && mounted != self.mounted;
        self.mounted = mounted;
        arrived.then_some(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SdCardMounted,
        })
    }

    fn observe_repair(&mut self, marker: Option<String>) -> Option<HostMessage> {
        let repair = marker
            .map(|marker| marker.trim().to_string())
            .filter(|marker| !marker.is_empty())?;
        if self.repair_shown.as_ref() == Some(&repair) {
            return None;
        }
        self.repair_shown = Some(repair);
        Some(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SdCardRepaired,
        })
    }
}

fn current_card(mount_point: &Path) -> Option<String> {
    let mountinfo = std::fs::read_to_string(MOUNTINFO_PATH).ok()?;
    mounted_card(&mountinfo, mount_point, |device| Path::new(device).exists())
}

/// The mount ID at `mount_point` while its device is still there; a pulled card leaves its
/// mount behind until the next card replaces it, but its device node goes at once.
fn mounted_card(
    mountinfo: &str,
    mount_point: &Path,
    device_present: impl Fn(&str) -> bool,
) -> Option<String> {
    mountinfo
        .lines()
        .filter_map(|line| {
            let (mount, filesystem) = line.split_once(" - ")?;
            let mut fields = mount.split_whitespace();
            let id = fields.next()?;
            let target = fields.nth(3)?;
            let device = filesystem.split_whitespace().nth(1)?;
            (Path::new(target) == mount_point && device_present(device)).then(|| id.to_string())
        })
        .last()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARD: &str = "/home/pi/samples/sd-card";

    fn mountinfo(id: u32) -> String {
        format!(
            "22 1 179:2 / / rw,noatime shared:1 - ext4 /dev/mmcblk0p2 rw\n\
             {id} 22 179:33 / {CARD} rw,nosuid shared:9 - vfat /dev/mmcblk1p1 rw,uid=1000\n"
        )
    }

    fn watch() -> SdCardWatch {
        SdCardWatch {
            mount_point: CARD.into(),
            mounted: None,
            repair_marker: "/nonexistent/octessera-test-repair-marker".into(),
            repair_shown: None,
            next_poll: Instant::now(),
        }
    }

    #[test]
    fn the_card_counts_as_mounted_only_while_its_device_is_present() {
        let mount_point = Path::new(CARD);
        assert_eq!(
            mounted_card(&mountinfo(41), mount_point, |_| true),
            Some("41".into())
        );
        assert_eq!(mounted_card(&mountinfo(41), mount_point, |_| false), None);
        assert_eq!(
            mounted_card(
                &mountinfo(41),
                Path::new("/var/lib/octessera/samples/sd-card"),
                |_| { true }
            ),
            None
        );
    }

    fn mounts(watch: &mut SdCardWatch, snapshots: &[String]) -> usize {
        snapshots
            .iter()
            .filter_map(|mountinfo| {
                watch.observe(mounted_card(mountinfo, Path::new(CARD), |_| true))
            })
            .filter(|message| {
                matches!(
                    message,
                    HostMessage::RuntimeResult {
                        result: RuntimeStoreResult::SdCardMounted
                    }
                )
            })
            .count()
    }

    #[test]
    fn a_pulled_card_reinserted_reports_one_mount_once_it_is_mounted_again() {
        let mut watch = watch();
        let boot_card_only = mountinfo(41).lines().next().unwrap().to_string();
        assert_eq!(mounts(&mut watch, &[mountinfo(41)]), 1);

        let pulled_then_checked_then_mounted = [
            boot_card_only.clone(),
            boot_card_only.clone(),
            boot_card_only,
            mountinfo(41),
            mountinfo(41),
        ];
        assert_eq!(mounts(&mut watch, &pulled_then_checked_then_mounted), 1);
    }

    fn repairs(watch: &mut SdCardWatch, markers: &[Option<&str>]) -> usize {
        markers
            .iter()
            .filter_map(|marker| watch.observe_repair(marker.map(str::to_string)))
            .filter(|message| {
                matches!(
                    message,
                    HostMessage::RuntimeResult {
                        result: RuntimeStoreResult::SdCardRepaired
                    }
                )
            })
            .count()
    }

    #[test]
    fn each_repair_is_reported_once() {
        let mut watch = watch();
        assert_eq!(repairs(&mut watch, &[None, Some(""), Some("\n")]), 0);
        assert_eq!(
            repairs(
                &mut watch,
                &[Some("/dev/mmcblk2p1 7\n"), Some("/dev/mmcblk2p1 7\n"), None]
            ),
            1
        );
        assert_eq!(repairs(&mut watch, &[Some("/dev/mmcblk2p1 7\n")]), 0);
        assert_eq!(repairs(&mut watch, &[Some("/dev/mmcblk2p1 9\n")]), 1);
    }

    #[test]
    fn a_new_mount_id_reports_another_mount() {
        let mut watch = watch();
        assert_eq!(mounts(&mut watch, &[mountinfo(41), mountinfo(57)]), 2);
    }
}
