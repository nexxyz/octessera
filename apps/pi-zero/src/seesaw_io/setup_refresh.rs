use std::time::{Duration, Instant};

/// One board's setup starts again per spacing, so each of the five gets it back about every 10 s.
pub(super) const SETUP_REFRESH_SPACING: Duration = Duration::from_secs(2);
const TRELLIS_BOARDS: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SeesawBoard {
    Trellis(usize),
    NeoKey,
}

/// A Seesaw that soft-resets quickly never fails a transaction, so its keypad and LED setup is
/// lost silently. The setup and then the board's pixels go out one board at a time and one short
/// part per worker pass, so a key scan only ever waits behind a couple of writes or one pixel
/// chunk.
pub(super) struct SetupRefresh {
    next_at: Instant,
    next_board: usize,
    part: Option<usize>,
}

impl SetupRefresh {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            next_at: now + SETUP_REFRESH_SPACING,
            next_board: 0,
            part: None,
        }
    }

    pub(super) fn take_due(&mut self, now: Instant) -> Option<(SeesawBoard, usize)> {
        if self.part.is_none() && now < self.next_at {
            return None;
        }
        let part = *self.part.get_or_insert(0);
        let board = if self.next_board < TRELLIS_BOARDS {
            SeesawBoard::Trellis(self.next_board)
        } else {
            SeesawBoard::NeoKey
        };
        Some((board, part))
    }

    /// A failed part ends that board's refresh; the failed board is set up again by its next scan.
    pub(super) fn record_part(&mut self, more: bool, now: Instant) {
        if more {
            self.part = self.part.map(|part| part + 1);
            return;
        }
        self.part = None;
        self.next_board = (self.next_board + 1) % (TRELLIS_BOARDS + 1);
        self.next_at = now + SETUP_REFRESH_SPACING;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_board(
        refresh: &mut SetupRefresh,
        now: Instant,
        parts: usize,
    ) -> Vec<(SeesawBoard, usize)> {
        (0..parts)
            .map(|index| {
                let due = refresh.take_due(now).expect("part due");
                refresh.record_part(index + 1 < parts, now);
                due
            })
            .collect()
    }

    #[test]
    fn each_board_gets_its_parts_in_turn_then_the_next_board_waits_its_spacing() {
        let start = Instant::now();
        let mut refresh = SetupRefresh::new(start);
        assert_eq!(refresh.take_due(start), None);
        assert_eq!(refresh.take_due(start + Duration::from_millis(1_999)), None);

        let mut now = start;
        let mut boards = Vec::new();
        for _ in 0..10 {
            now += SETUP_REFRESH_SPACING;
            let parts = run_board(&mut refresh, now, 3);
            assert_eq!(
                parts.iter().map(|(_, part)| *part).collect::<Vec<_>>(),
                [0, 1, 2]
            );
            assert!(parts.iter().all(|(board, _)| *board == parts[0].0));
            boards.push(parts[0].0);
            assert_eq!(
                refresh.take_due(now + SETUP_REFRESH_SPACING - Duration::from_millis(1)),
                None
            );
        }

        let cycle = [
            SeesawBoard::Trellis(0),
            SeesawBoard::Trellis(1),
            SeesawBoard::Trellis(2),
            SeesawBoard::Trellis(3),
            SeesawBoard::NeoKey,
        ];
        assert_eq!(boards[..5], cycle);
        assert_eq!(boards[5..], cycle);
    }

    #[test]
    fn a_started_board_continues_on_the_next_pass_without_waiting() {
        let start = Instant::now();
        let mut refresh = SetupRefresh::new(start);
        let due = start + SETUP_REFRESH_SPACING;
        assert_eq!(refresh.take_due(due), Some((SeesawBoard::Trellis(0), 0)));
        refresh.record_part(true, due);
        assert_eq!(
            refresh.take_due(due + Duration::from_millis(2)),
            Some((SeesawBoard::Trellis(0), 1))
        );
    }

    #[test]
    fn a_failed_part_moves_on_to_the_next_board_after_the_spacing() {
        let start = Instant::now();
        let mut refresh = SetupRefresh::new(start);
        let due = start + SETUP_REFRESH_SPACING;
        assert_eq!(refresh.take_due(due), Some((SeesawBoard::Trellis(0), 0)));
        refresh.record_part(false, due);
        assert_eq!(refresh.take_due(due + Duration::from_millis(2)), None);
        assert_eq!(
            refresh.take_due(due + SETUP_REFRESH_SPACING),
            Some((SeesawBoard::Trellis(1), 0))
        );
    }
}
