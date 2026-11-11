use std::time::Duration;

/// How long the desktop output may go without pulling a block before it counts as lost.
pub(crate) const OUTPUT_CHECK_INTERVAL: Duration = Duration::from_secs(1);

/// Notices a desktop output that stopped pulling audio, such as an unplugged device: rodio
/// only logs its stream errors. Checked once per [`OUTPUT_CHECK_INTERVAL`].
#[derive(Default)]
pub(crate) struct OutputWatch {
    last_pulls: u64,
    stalled: bool,
}

impl OutputWatch {
    /// `true` the first time a check finds no block pulled since the previous one.
    pub(crate) fn newly_stalled(&mut self, pulls: u64) -> bool {
        let moved = pulls != self.last_pulls;
        self.last_pulls = pulls;
        if moved {
            self.stalled = false;
            return false;
        }
        !std::mem::replace(&mut self.stalled, true)
    }

    /// The output could not be opened at all.
    pub(crate) fn lost(&mut self) {
        self.stalled = true;
    }

    pub(crate) fn needs_reopen(&self) -> bool {
        self.stalled
    }

    pub(crate) fn reopened(&mut self) {
        self.stalled = false;
    }
}

#[cfg(test)]
mod tests {
    use super::OutputWatch;

    #[test]
    fn a_pulling_output_never_counts_as_stalled() {
        let mut watch = OutputWatch::default();
        for pulls in [3, 9, 20] {
            assert!(!watch.newly_stalled(pulls));
        }
        assert!(!watch.needs_reopen());
    }

    #[test]
    fn an_output_that_stops_pulling_is_reported_once_and_reopened_on_request() {
        let mut watch = OutputWatch::default();
        assert!(!watch.newly_stalled(40));

        assert!(watch.newly_stalled(40));
        assert!(!watch.newly_stalled(40));
        assert!(watch.needs_reopen());

        watch.reopened();
        assert!(!watch.needs_reopen());
        assert!(!watch.newly_stalled(41));
    }

    #[test]
    fn an_output_that_resumes_by_itself_needs_no_reopen() {
        let mut watch = OutputWatch::default();
        watch.newly_stalled(5);
        assert!(watch.newly_stalled(5));

        assert!(!watch.newly_stalled(6));
        assert!(!watch.needs_reopen());
    }
}
