//! Notices the OLED board's SD card (SD2) being mounted, unmounted or swapped, so the runtime
//! resolves the patch's card samples again.

use playback_runtime::{HostMessage, RuntimeStoreResult};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_secs(1);
const MOUNTINFO_PATH: &str = "/proc/self/mountinfo";

pub(crate) struct SdCardWatch {
    mount_point: PathBuf,
    mounted: Option<String>,
    next_poll: Instant,
}

impl SdCardWatch {
    pub(crate) fn new(samples_dir: &Path) -> Self {
        let mount_point = samples_dir.join(crate::sample_browser::SD_CARD_SAMPLE_DIR);
        Self {
            mounted: current_card(&mount_point),
            mount_point,
            next_poll: Instant::now() + POLL_INTERVAL,
        }
    }

    pub(crate) fn poll(&mut self, now: Instant) -> Option<HostMessage> {
        if now < self.next_poll {
            return None;
        }
        self.next_poll = now + POLL_INTERVAL;
        self.observe(current_card(&self.mount_point))
    }

    fn observe(&mut self, mounted: Option<String>) -> Option<HostMessage> {
        (mounted != self.mounted).then(|| {
            self.mounted = mounted;
            HostMessage::RuntimeResult {
                result: RuntimeStoreResult::SdCardChanged,
            }
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

    #[test]
    fn mounting_unmounting_and_swapping_the_card_each_report_one_change() {
        let mut watch = watch();
        let changed = |message: Option<HostMessage>| {
            matches!(
                message,
                Some(HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::SdCardChanged
                })
            )
        };

        assert!(watch.observe(None).is_none());
        assert!(changed(watch.observe(Some("41".into()))));
        assert!(watch.observe(Some("41".into())).is_none());
        assert!(changed(watch.observe(Some("57".into()))));
        assert!(changed(watch.observe(None)));
        assert!(watch.observe(None).is_none());
    }
}
