use std::time::{Duration, Instant};

/// One board's setup is re-applied per spacing, so each of the five gets it back every 10 s.
pub(super) const SETUP_REFRESH_SPACING: Duration = Duration::from_secs(2);
const TRELLIS_BOARDS: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SeesawBoard {
    Trellis(usize),
    NeoKey,
}

/// A Seesaw that soft-resets quickly never fails a transaction, so its keypad and LED setup is
/// lost silently; re-applying it one board at a time spreads the bus burst out.
pub(super) struct SetupRefresh {
    next_at: Instant,
    next_board: usize,
}

impl SetupRefresh {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            next_at: now + SETUP_REFRESH_SPACING,
            next_board: 0,
        }
    }

    pub(super) fn take_due(&mut self, now: Instant) -> Option<SeesawBoard> {
        if now < self.next_at {
            return None;
        }
        let board = if self.next_board < TRELLIS_BOARDS {
            SeesawBoard::Trellis(self.next_board)
        } else {
            SeesawBoard::NeoKey
        };
        self.next_board = (self.next_board + 1) % (TRELLIS_BOARDS + 1);
        self.next_at = now + SETUP_REFRESH_SPACING;
        Some(board)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_board_is_set_up_again_once_every_ten_seconds_one_at_a_time() {
        let start = Instant::now();
        let mut refresh = SetupRefresh::new(start);
        assert_eq!(refresh.take_due(start), None);
        assert_eq!(refresh.take_due(start + Duration::from_millis(1_999)), None);

        let boards: Vec<_> = (1..=10)
            .map(|step| {
                let now = start + SETUP_REFRESH_SPACING * step;
                let board = refresh.take_due(now);
                assert_eq!(refresh.take_due(now), None);
                board
            })
            .collect();

        let cycle = [
            Some(SeesawBoard::Trellis(0)),
            Some(SeesawBoard::Trellis(1)),
            Some(SeesawBoard::Trellis(2)),
            Some(SeesawBoard::Trellis(3)),
            Some(SeesawBoard::NeoKey),
        ];
        assert_eq!(boards[..5], cycle);
        assert_eq!(boards[5..], cycle);
    }

    #[test]
    fn a_late_worker_pass_does_not_bunch_the_next_boards_up() {
        let start = Instant::now();
        let mut refresh = SetupRefresh::new(start);
        let late = start + Duration::from_secs(30);
        assert_eq!(refresh.take_due(late), Some(SeesawBoard::Trellis(0)));
        assert_eq!(refresh.take_due(late + Duration::from_millis(1)), None);
        assert_eq!(
            refresh.take_due(late + SETUP_REFRESH_SPACING),
            Some(SeesawBoard::Trellis(1))
        );
    }
}
