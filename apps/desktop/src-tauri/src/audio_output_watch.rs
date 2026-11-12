use std::time::Duration;

/// How long the desktop output may go without pulling a block before it counts as lost.
pub(crate) const OUTPUT_CHECK_INTERVAL: Duration = Duration::from_secs(1);

/// What one check found out about the output.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum OutputCheck {
    Unchanged,
    /// No block was pulled since the previous check, for the first time.
    Stalled,
    /// A stalled output pulled again by itself.
    Resumed,
}

/// Notices a desktop output that stopped pulling audio, such as an unplugged device: rodio
/// only logs its stream errors. Checked once per [`OUTPUT_CHECK_INTERVAL`].
#[derive(Default)]
pub(crate) struct OutputWatch {
    last_pulls: u64,
    stalled: bool,
}

impl OutputWatch {
    pub(crate) fn check(&mut self, pulls: u64) -> OutputCheck {
        let moved = pulls != self.last_pulls;
        self.last_pulls = pulls;
        match (moved, std::mem::replace(&mut self.stalled, !moved)) {
            (true, true) => OutputCheck::Resumed,
            (false, false) => OutputCheck::Stalled,
            _ => OutputCheck::Unchanged,
        }
    }

    /// The output could not be opened at all.
    pub(crate) fn lost(&mut self) {
        self.stalled = true;
    }

    /// Answers a reopen request: a lost output is replaced by a fresh `open`. `true` when the
    /// output is healthy afterwards, whether it was reopened or never lost.
    pub(crate) fn serve_reopen<T>(
        &mut self,
        output: &mut Option<T>,
        open: impl FnOnce() -> Option<T>,
    ) -> bool {
        if self.stalled {
            drop(output.take());
            *output = open();
            self.stalled = output.is_none();
        }
        !self.stalled
    }
}

#[cfg(test)]
mod tests {
    use super::{OutputCheck, OutputWatch};

    #[test]
    fn a_pulling_output_never_counts_as_stalled() {
        let mut watch = OutputWatch::default();
        for pulls in [3, 9, 20] {
            assert_eq!(watch.check(pulls), OutputCheck::Unchanged);
        }
    }

    #[test]
    fn an_output_that_stops_pulling_is_reported_once_and_reopened_on_request() {
        let mut watch = OutputWatch::default();
        assert_eq!(watch.check(40), OutputCheck::Unchanged);

        assert_eq!(watch.check(40), OutputCheck::Stalled);
        assert_eq!(watch.check(40), OutputCheck::Unchanged);

        let mut output = Some(1);
        assert!(!watch.serve_reopen(&mut output, || None));
        assert_eq!(output, None);
        assert!(watch.serve_reopen(&mut output, || Some(2)));
        assert_eq!(output, Some(2));
        assert_eq!(watch.check(41), OutputCheck::Unchanged);
    }

    #[test]
    fn a_reopen_request_on_a_healthy_output_keeps_it_and_reports_it_healthy() {
        let mut watch = OutputWatch::default();
        assert_eq!(watch.check(3), OutputCheck::Unchanged);
        let mut output = Some(1);

        assert!(watch.serve_reopen(&mut output, || panic!("a healthy output is not reopened")));
        assert_eq!(output, Some(1));
    }

    #[test]
    fn an_output_that_never_opened_is_opened_on_request() {
        let mut watch = OutputWatch::default();
        watch.lost();
        let mut output = None;

        assert!(watch.serve_reopen(&mut output, || Some(1)));
        assert_eq!(output, Some(1));
    }

    #[test]
    fn an_output_that_resumes_by_itself_is_reported_once_and_needs_no_reopen() {
        let mut watch = OutputWatch::default();
        watch.check(5);
        assert_eq!(watch.check(5), OutputCheck::Stalled);

        assert_eq!(watch.check(6), OutputCheck::Resumed);
        assert_eq!(watch.check(7), OutputCheck::Unchanged);
        let mut output = Some(1);
        assert!(watch.serve_reopen(&mut output, || panic!("a resumed output is not reopened")));
    }
}
