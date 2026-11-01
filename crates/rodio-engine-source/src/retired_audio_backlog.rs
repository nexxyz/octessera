use super::{drop_retired_item, RetiredAudioItem, RETIREMENT_BACKLOG_CAPACITY};
use crossbeam_channel::{Receiver, Sender, TrySendError};
use realtime_engine::synth::RetiredAudioState;

pub(crate) struct RetiredAudioBacklog {
    pub(crate) items: Box<[Option<RetiredAudioItem>]>,
    pub(crate) read: usize,
    pub(crate) write: usize,
    pub(crate) len: usize,
}

impl RetiredAudioBacklog {
    pub(crate) fn new() -> Self {
        let mut items: Box<[Option<RetiredAudioItem>]> =
            std::iter::repeat_with(|| Some(full_size_item()))
                .take(RETIREMENT_BACKLOG_CAPACITY)
                .collect();
        items.iter_mut().for_each(|slot| *slot = None);
        Self {
            items,
            read: 0,
            write: 0,
            len: 0,
        }
    }

    pub(crate) fn flush(
        &mut self,
        retired_tx: &Sender<RetiredAudioItem>,
        retirement_disconnected: &mut bool,
    ) {
        if *retirement_disconnected {
            return;
        }
        while self.len > 0 {
            let Some(item) = self.items[self.read].take() else {
                self.len = 0;
                break;
            };
            match retired_tx.try_send(item) {
                Ok(()) => {
                    self.read = (self.read + 1) % RETIREMENT_BACKLOG_CAPACITY;
                    self.len -= 1;
                }
                Err(TrySendError::Full(item)) => {
                    self.items[self.read] = Some(item);
                    break;
                }
                Err(TrySendError::Disconnected(item)) => {
                    self.items[self.read] = Some(item);
                    *retirement_disconnected = true;
                    break;
                }
            }
        }
    }

    pub(crate) fn enqueue(&mut self, item: RetiredAudioItem) -> bool {
        if self.len >= RETIREMENT_BACKLOG_CAPACITY {
            return false;
        }
        self.items[self.write] = Some(item);
        self.write = (self.write + 1) % RETIREMENT_BACKLOG_CAPACITY;
        self.len += 1;
        true
    }

    pub(crate) fn drain(mut self) {
        while self.len > 0 {
            let Some(item) = self.items[self.read].take() else {
                self.len = 0;
                break;
            };
            self.read = (self.read + 1) % RETIREMENT_BACKLOG_CAPACITY;
            self.len -= 1;
            drop_retired_item(item);
        }
    }
}

pub(crate) fn prefault_retirement_ring(
    retired_tx: &Sender<RetiredAudioItem>,
    retired_rx: &Receiver<RetiredAudioItem>,
) -> usize {
    let mut written = 0;
    while retired_tx.try_send(full_size_item()).is_ok() {
        written += 1;
    }
    retired_rx.try_iter().for_each(drop_retired_item);
    written
}

// Writing full-size items pre-faults retirement storage: mlockall(MCL_ONFAULT) would otherwise fault its pages in the audio callback.
fn full_size_item() -> RetiredAudioItem {
    RetiredAudioItem {
        state: Some(RetiredAudioState::default()),
        event: None,
        #[cfg(test)]
        drop_probe: None,
    }
}
