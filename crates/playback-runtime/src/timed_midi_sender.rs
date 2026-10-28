use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub type MidiSink = Box<dyn FnMut(&[u8]) -> Result<(), String> + Send>;

const SENDER_STOPPED: &str = "MIDI sender stopped";

enum Command {
    Send {
        at: Option<Instant>,
        bytes: Vec<u8>,
    },
    Flush,
    Panic {
        done: SyncSender<Result<(), String>>,
    },
    SetSink(Option<MidiSink>),
}

struct Worker {
    tx: Sender<Command>,
    error: Arc<Mutex<Option<String>>>,
}

/// Sends MIDI from one thread per output, holding timestamped messages until they are due.
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
        worker
            .tx
            .send(Command::Send {
                at,
                bytes: bytes.to_vec(),
            })
            .map_err(|_| SENDER_STOPPED.to_string())?;
        match worker.error.lock().map(|mut error| error.take()) {
            Ok(Some(error)) => Err(error),
            _ => Ok(()),
        }
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
        result.recv().unwrap_or_else(|_| Err(SENDER_STOPPED.into()))
    }

    fn command(&mut self, command: Command) -> Result<(), String> {
        if self.worker.is_none() {
            self.worker = Some(self.spawn()?);
        }
        let Some(worker) = &self.worker else {
            return Err(SENDER_STOPPED.into());
        };
        worker
            .tx
            .send(command)
            .map_err(|_| SENDER_STOPPED.to_string())
    }

    fn spawn(&self) -> Result<Worker, String> {
        let (tx, rx) = mpsc::channel();
        let error = Arc::new(Mutex::new(None));
        let thread_error = error.clone();
        let on_start = self.on_start;
        std::thread::Builder::new()
            .name(self.thread_name.into())
            .spawn(move || {
                on_start();
                run(&rx, &thread_error);
            })
            .map_err(|error| format!("MIDI sender thread failed to start: {error}"))?;
        Ok(Worker { tx, error })
    }
}

fn midi_panic_messages() -> impl Iterator<Item = Vec<u8>> {
    std::iter::once(vec![0xFC]).chain(
        (0..16_u8).flat_map(|channel| [vec![0xB0 | channel, 120, 0], vec![0xB0 | channel, 123, 0]]),
    )
}

fn emit(
    sink: &mut Option<MidiSink>,
    bytes: &[u8],
    error: &Mutex<Option<String>>,
) -> Result<(), String> {
    let Some(sink) = sink.as_mut() else {
        return Ok(());
    };
    let result = sink(bytes);
    if let Err(message) = &result {
        if let Ok(mut slot) = error.lock() {
            slot.get_or_insert_with(|| message.clone());
        }
    }
    result
}

fn run(rx: &Receiver<Command>, error: &Mutex<Option<String>>) {
    let mut schedule = MidiSchedule::default();
    let mut sink: Option<MidiSink> = None;
    loop {
        let command = match schedule.next_due() {
            None => match rx.recv() {
                Ok(command) => command,
                Err(_) => return,
            },
            Some(due) => {
                let now = Instant::now();
                if let Some(bytes) = schedule.pop_due(now) {
                    let _ = emit(&mut sink, &bytes, error);
                    continue;
                }
                match rx.recv_timeout(due.saturating_duration_since(now)) {
                    Ok(command) => command,
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        };
        match command {
            Command::Send { at: None, bytes } => {
                let _ = emit(&mut sink, &bytes, error);
            }
            Command::Send {
                at: Some(at),
                bytes,
            } => schedule.push(at, bytes),
            Command::Flush => {
                for bytes in schedule.take_note_offs() {
                    let _ = emit(&mut sink, &bytes, error);
                }
            }
            Command::Panic { done } => {
                schedule.clear();
                let mut first_error = None;
                for message in midi_panic_messages() {
                    if let Err(failure) = emit(&mut sink, &message, error) {
                        first_error.get_or_insert(failure);
                    }
                }
                let _ = done.send(first_error.map_or(Ok(()), Err));
            }
            Command::SetSink(next) => {
                for bytes in schedule.take_note_offs() {
                    let _ = emit(&mut sink, &bytes, error);
                }
                sink = next;
            }
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
