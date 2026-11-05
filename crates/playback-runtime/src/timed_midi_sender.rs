use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

pub type MidiSink = Box<dyn FnMut(&[u8]) -> Result<(), String> + Send>;

const SENDER_STOPPED: &str = "MIDI sender stopped";

enum Command {
    Send { at: Option<Instant>, bytes: Vec<u8> },
    Flush,
    Panic { done: SyncSender<()> },
    SetSink(Option<MidiSink>),
}

/// How the sender thread sleeps. On the boards it outranks the runtime thread on the same
/// core, so a wake preempts the runtime; a message it would reach in time anyway does not
/// wake it.
#[derive(Clone, Copy)]
enum Sleep {
    Awake,
    UntilMessage,
    Until(Instant),
}

struct Inbox {
    commands: Vec<Command>,
    sleep: Sleep,
    stopped: bool,
}

struct Shared {
    inbox: Mutex<Inbox>,
    wake: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Inbox> {
        self.inbox.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Queues a command due at `due` (now when `None`).
    fn post(&self, command: Command, due: Option<Instant>) -> Result<(), String> {
        let mut inbox = self.lock();
        if inbox.stopped {
            return Err(SENDER_STOPPED.into());
        }
        inbox.commands.push(command);
        let wake = match inbox.sleep {
            Sleep::Awake => false,
            Sleep::UntilMessage => true,
            Sleep::Until(deadline) => due.is_none_or(|due| due < deadline),
        };
        if wake {
            inbox.sleep = Sleep::Awake;
            drop(inbox);
            self.wake.notify_one();
        }
        Ok(())
    }

    fn stop(&self) {
        let mut inbox = self.lock();
        inbox.stopped = true;
        inbox.commands.clear();
        drop(inbox);
        self.wake.notify_one();
    }
}

struct Worker {
    shared: Arc<Shared>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.shared.stop();
    }
}

/// Marks the sender stopped when its thread ends, also by a panic in an output.
struct StopOnExit(Arc<Shared>);

impl Drop for StopOnExit {
    fn drop(&mut self) {
        self.0.stop();
    }
}

/// Sends MIDI from one thread per output, holding timestamped messages until they are due.
/// A failing output (a device unplugged, or a host that stopped reading) is logged, not
/// reported: plugging and unplugging MIDI is normal use.
pub struct TimedMidiSender {
    thread_name: &'static str,
    on_start: fn(),
    worker: Option<Worker>,
}

impl TimedMidiSender {
    pub fn new(thread_name: &'static str, on_start: fn()) -> Self {
        Self {
            thread_name,
            on_start,
            worker: None,
        }
    }

    pub fn set_sink(&mut self, sink: Option<MidiSink>) -> Result<(), String> {
        if sink.is_none() && self.worker.is_none() {
            return Ok(());
        }
        self.command(Command::SetSink(sink))
    }

    pub fn send(&mut self, at: Option<Instant>, bytes: &[u8]) -> Result<(), String> {
        let Some(worker) = &self.worker else {
            return Ok(());
        };
        let bytes = bytes.to_vec();
        worker.shared.post(Command::Send { at, bytes }, at)
    }

    pub fn flush(&mut self) -> Result<(), String> {
        if self.worker.is_none() {
            return Ok(());
        }
        self.command(Command::Flush)
    }

    pub fn panic(&mut self) -> Result<(), String> {
        if self.worker.is_none() {
            return Ok(());
        }
        let (done, result) = mpsc::sync_channel(1);
        self.command(Command::Panic { done })?;
        result.recv().map_err(|_| SENDER_STOPPED.into())
    }

    fn command(&mut self, command: Command) -> Result<(), String> {
        if self.worker.is_none() {
            self.worker = Some(self.spawn()?);
        }
        let Some(worker) = &self.worker else {
            return Err(SENDER_STOPPED.into());
        };
        worker.shared.post(command, None)
    }

    fn spawn(&self) -> Result<Worker, String> {
        let shared = Arc::new(Shared {
            inbox: Mutex::new(Inbox {
                commands: Vec::new(),
                sleep: Sleep::Awake,
                stopped: false,
            }),
            wake: Condvar::new(),
        });
        let thread_shared = Arc::clone(&shared);
        let on_start = self.on_start;
        std::thread::Builder::new()
            .name(self.thread_name.into())
            .spawn(move || {
                let _stop = StopOnExit(Arc::clone(&thread_shared));
                on_start();
                run(&thread_shared);
            })
            .map_err(|error| format!("MIDI sender thread failed to start: {error}"))?;
        Ok(Worker { shared })
    }
}

fn midi_panic_messages() -> impl Iterator<Item = Vec<u8>> {
    std::iter::once(vec![0xFC]).chain(
        (0..16_u8).flat_map(|channel| [vec![0xB0 | channel, 120, 0], vec![0xB0 | channel, 123, 0]]),
    )
}

#[derive(Default)]
struct Output {
    sink: Option<MidiSink>,
    failing: bool,
}

impl Output {
    fn emit(&mut self, bytes: &[u8]) {
        let Some(sink) = self.sink.as_mut() else {
            return;
        };
        match sink(bytes) {
            Ok(()) => self.failing = false,
            Err(error) if !self.failing => {
                self.failing = true;
                eprintln!("MIDI output is not taking messages: {error}");
            }
            Err(_) => {}
        }
    }

    fn replace(&mut self, sink: Option<MidiSink>) {
        self.sink = sink;
        self.failing = false;
    }
}

fn run(shared: &Shared) {
    let mut schedule = MidiSchedule::default();
    let mut output = Output::default();
    loop {
        if let Some(bytes) = schedule.pop_due(Instant::now()) {
            output.emit(&bytes);
            continue;
        }
        let mut inbox = shared.lock();
        if inbox.stopped {
            return;
        }
        if inbox.commands.is_empty() {
            let due = schedule.next_due();
            inbox.sleep = due.map_or(Sleep::UntilMessage, Sleep::Until);
            let mut inbox = match due {
                None => shared
                    .wake
                    .wait(inbox)
                    .unwrap_or_else(PoisonError::into_inner),
                Some(due) => {
                    shared
                        .wake
                        .wait_timeout(inbox, due.saturating_duration_since(Instant::now()))
                        .unwrap_or_else(PoisonError::into_inner)
                        .0
                }
            };
            inbox.sleep = Sleep::Awake;
            continue;
        }
        let commands = std::mem::take(&mut inbox.commands);
        drop(inbox);
        for command in commands {
            handle(command, &mut schedule, &mut output);
        }
    }
}

fn handle(command: Command, schedule: &mut MidiSchedule, output: &mut Output) {
    match command {
        Command::Send { at: None, bytes } => output.emit(&bytes),
        Command::Send {
            at: Some(at),
            bytes,
        } => schedule.push(at, bytes),
        Command::Flush => {
            for bytes in schedule.take_note_offs() {
                output.emit(&bytes);
            }
        }
        Command::Panic { done } => {
            schedule.clear();
            for message in midi_panic_messages() {
                output.emit(&message);
            }
            let _ = done.send(());
        }
        Command::SetSink(next) => {
            for bytes in schedule.take_note_offs() {
                output.emit(&bytes);
            }
            output.replace(next);
        }
    }
}

#[derive(Default)]
struct MidiSchedule {
    heap: BinaryHeap<Reverse<(Instant, u64, Vec<u8>)>>,
    next_seq: u64,
}

impl MidiSchedule {
    fn push(&mut self, at: Instant, bytes: Vec<u8>) {
        self.heap.push(Reverse((at, self.next_seq, bytes)));
        self.next_seq = self.next_seq.wrapping_add(1);
    }

    fn next_due(&self) -> Option<Instant> {
        self.heap.peek().map(|Reverse((at, _, _))| *at)
    }

    fn pop_due(&mut self, now: Instant) -> Option<Vec<u8>> {
        if self.next_due()? > now {
            return None;
        }
        self.heap.pop().map(|Reverse((_, _, bytes))| bytes)
    }

    fn take_note_offs(&mut self) -> Vec<Vec<u8>> {
        let mut pending = std::mem::take(&mut self.heap).into_sorted_vec();
        pending.reverse();
        pending
            .into_iter()
            .map(|Reverse((_, _, bytes))| bytes)
            .filter(|bytes| is_note_off(bytes))
            .collect()
    }

    fn clear(&mut self) {
        self.heap.clear();
    }
}

fn is_note_off(bytes: &[u8]) -> bool {
    match bytes {
        [status, ..] if status & 0xF0 == 0x80 => true,
        [status, _, 0, ..] => status & 0xF0 == 0x90,
        _ => false,
    }
}

#[cfg(test)]
mod tests;
