use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::time::Instant;

pub type MidiSink = Box<dyn FnMut(&[u8]) -> Result<(), String> + Send>;

const SENDER_STOPPED: &str = "MIDI sender stopped";

enum Command {
    Send { at: Option<Instant>, bytes: Vec<u8> },
    Flush,
    Panic { done: SyncSender<()> },
    SetSink(Option<MidiSink>),
}

struct Worker {
    tx: Sender<Command>,
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
        worker
            .tx
            .send(Command::Send {
                at,
                bytes: bytes.to_vec(),
            })
            .map_err(|_| SENDER_STOPPED.to_string())
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
        worker
            .tx
            .send(command)
            .map_err(|_| SENDER_STOPPED.to_string())
    }

    fn spawn(&self) -> Result<Worker, String> {
        let (tx, rx) = mpsc::channel();
        let on_start = self.on_start;
        std::thread::Builder::new()
            .name(self.thread_name.into())
            .spawn(move || {
                on_start();
                run(&rx);
            })
            .map_err(|error| format!("MIDI sender thread failed to start: {error}"))?;
        Ok(Worker { tx })
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

fn run(rx: &Receiver<Command>) {
    let mut schedule = MidiSchedule::default();
    let mut output = Output::default();
    loop {
        let command = match schedule.next_due() {
            None => match rx.recv() {
                Ok(command) => command,
                Err(_) => return,
            },
            Some(due) => {
                let now = Instant::now();
                if let Some(bytes) = schedule.pop_due(now) {
                    output.emit(&bytes);
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
