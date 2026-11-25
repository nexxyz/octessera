//! Queues between the SCHED_FIFO runtime thread and lower-priority threads on its CPU. Unlike
//! `std::sync::mpsc`, nothing here waits in user space for a peer to finish a push or pop: the
//! runtime's polling receive gives up on a contended lock, and every wait blocks in the kernel,
//! so a preempted peer gets the CPU back.

use std::collections::VecDeque;
use std::sync::mpsc::{RecvError, RecvTimeoutError, SendError, TryRecvError, TrySendError};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, TryLockError};
use std::time::{Duration, Instant};

struct State<T> {
    items: VecDeque<T>,
    senders: usize,
    waiting_senders: usize,
    receiver: bool,
}

struct Shared<T> {
    state: Mutex<State<T>>,
    capacity: usize,
    arrived: Condvar,
    space: Condvar,
}

impl<T> Shared<T> {
    fn lock(&self) -> MutexGuard<'_, State<T>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn take(&self, state: &mut State<T>) -> Option<T> {
        let item = state.items.pop_front();
        if item.is_some() && state.waiting_senders > 0 {
            self.space.notify_one();
        }
        item
    }
}

pub(crate) struct Sender<T> {
    shared: Arc<Shared<T>>,
}

pub(crate) struct Receiver<T> {
    shared: Arc<Shared<T>>,
}

pub(crate) fn channel<T>() -> (Sender<T>, Receiver<T>) {
    with_capacity(usize::MAX)
}

pub(crate) fn bounded<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    assert!(
        capacity > 0,
        "a runtime handoff queue needs room for one item"
    );
    with_capacity(capacity)
}

fn with_capacity<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let shared = Arc::new(Shared {
        state: Mutex::new(State {
            items: VecDeque::new(),
            senders: 1,
            waiting_senders: 0,
            receiver: true,
        }),
        capacity,
        arrived: Condvar::new(),
        space: Condvar::new(),
    });
    (
        Sender {
            shared: shared.clone(),
        },
        Receiver { shared },
    )
}

impl<T> Sender<T> {
    /// Waits while a bounded queue is full.
    pub(crate) fn send(&self, value: T) -> Result<(), SendError<T>> {
        let mut state = self.shared.lock();
        while state.receiver && state.items.len() >= self.shared.capacity {
            state.waiting_senders += 1;
            state = self
                .shared
                .space
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
            state.waiting_senders -= 1;
        }
        if !state.receiver {
            return Err(SendError(value));
        }
        state.items.push_back(value);
        drop(state);
        self.shared.arrived.notify_one();
        Ok(())
    }

    pub(crate) fn try_send(&self, value: T) -> Result<(), TrySendError<T>> {
        let mut state = self.shared.lock();
        if !state.receiver {
            return Err(TrySendError::Disconnected(value));
        }
        if state.items.len() >= self.shared.capacity {
            return Err(TrySendError::Full(value));
        }
        state.items.push_back(value);
        drop(state);
        self.shared.arrived.notify_one();
        Ok(())
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        self.shared.lock().senders += 1;
        Self {
            shared: self.shared.clone(),
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let mut state = self.shared.lock();
        state.senders -= 1;
        if state.senders == 0 {
            drop(state);
            self.shared.arrived.notify_all();
        }
    }
}

impl<T> Receiver<T> {
    /// Reports `Empty` while a sender holds the queue, so the caller never waits for it.
    pub(crate) fn try_recv(&self) -> Result<T, TryRecvError> {
        let mut state = match self.shared.state.try_lock() {
            Ok(state) => state,
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(TryLockError::WouldBlock) => return Err(TryRecvError::Empty),
        };
        match self.shared.take(&mut state) {
            Some(item) => Ok(item),
            None if state.senders == 0 => Err(TryRecvError::Disconnected),
            None => Err(TryRecvError::Empty),
        }
    }

    pub(crate) fn try_iter(&self) -> impl Iterator<Item = T> + '_ {
        std::iter::from_fn(|| self.try_recv().ok())
    }

    /// Takes everything queued now, waiting for the lock instead of giving up on it.
    pub(crate) fn drain_queued(&self) -> Vec<T> {
        let mut state = self.shared.lock();
        std::iter::from_fn(|| self.shared.take(&mut state)).collect()
    }

    pub(crate) fn recv(&self) -> Result<T, RecvError> {
        let mut state = self.shared.lock();
        loop {
            if let Some(item) = self.shared.take(&mut state) {
                return Ok(item);
            }
            if state.senders == 0 {
                return Err(RecvError);
            }
            state = self
                .shared
                .arrived
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    pub(crate) fn recv_timeout(&self, timeout: Duration) -> Result<T, RecvTimeoutError> {
        let Some(deadline) = Instant::now().checked_add(timeout) else {
            return self.recv().map_err(|_| RecvTimeoutError::Disconnected);
        };
        let mut state = self.shared.lock();
        loop {
            if let Some(item) = self.shared.take(&mut state) {
                return Ok(item);
            }
            if state.senders == 0 {
                return Err(RecvTimeoutError::Disconnected);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(RecvTimeoutError::Timeout);
            }
            state = self
                .shared
                .arrived
                .wait_timeout(state, remaining)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        let mut state = self.shared.lock();
        state.receiver = false;
        let items = std::mem::take(&mut state.items);
        drop(state);
        self.shared.space.notify_all();
        drop(items);
    }
}

#[cfg(test)]
mod tests;
