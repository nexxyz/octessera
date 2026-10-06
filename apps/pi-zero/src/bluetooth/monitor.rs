//! Bluetooth audio monitor: a low-priority copy of the final mix played on a
//! connected speaker through bluez-alsa. The primary callback only fills a
//! bounded tap; one background thread hands it to `aplay`, so a slow or
//! vanished speaker drops monitor audio and never touches the wired outputs.

use crate::audio::MixTapState;
use media_recording::RecordingChunk;
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CHUNK_WAIT: Duration = Duration::from_millis(200);
const RESTART_DELAY: Duration = Duration::from_secs(2);
const PLAYER_BUFFER_MICROS: u32 = 200_000;

pub(crate) struct BluetoothAudioMonitor {
    address: String,
    taps: MixTapState,
    stop: Arc<AtomicBool>,
    player: Arc<Mutex<Option<Child>>>,
}

impl BluetoothAudioMonitor {
    pub(crate) fn start(address: &str, taps: MixTapState, sample_rate: u32) -> Self {
        let (tap, chunks) = media_recording::monitor_tap();
        if let Ok(mut taps) = taps.write() {
            taps.monitor = Some(tap);
        }
        let monitor = Self {
            address: address.into(),
            taps,
            stop: Arc::new(AtomicBool::new(false)),
            player: Arc::new(Mutex::new(None)),
        };
        let feeder = Feeder {
            address: address.into(),
            sample_rate,
            chunks,
            stop: monitor.stop.clone(),
            player: monitor.player.clone(),
        };
        if let Err(error) = std::thread::Builder::new()
            .name("octessera-bt-audio".into())
            .spawn(move || feeder.run())
        {
            eprintln!("Bluetooth audio feeder unavailable: {error}");
        }
        monitor
    }

    pub(crate) fn address(&self) -> &str {
        &self.address
    }
}

impl Drop for BluetoothAudioMonitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Ok(mut taps) = self.taps.write() {
            taps.monitor = None;
        }
        if let Ok(mut player) = self.player.lock() {
            if let Some(mut child) = player.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

struct Feeder {
    address: String,
    sample_rate: u32,
    chunks: Receiver<RecordingChunk>,
    stop: Arc<AtomicBool>,
    player: Arc<Mutex<Option<Child>>>,
}

impl Feeder {
    fn run(self) {
        lower_priority();
        let mut start_failure_reported = false;
        while !self.stopped() {
            match self.spawn_player() {
                Ok(()) => {
                    if self.pump() == Pump::Disconnected {
                        break;
                    }
                }
                Err(error) if !start_failure_reported => {
                    eprintln!("Bluetooth audio player failed to start: {error}");
                    start_failure_reported = true;
                }
                Err(_) => {}
            }
            self.stop_player();
            if !self.discard_until(Instant::now() + RESTART_DELAY) {
                break;
            }
        }
        self.stop_player();
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

    fn spawn_player(&self) -> std::io::Result<()> {
        let child = Command::new("aplay")
            .args(player_args(&self.address, self.sample_rate))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()?;
        let mut player = self
            .player
            .lock()
            .map_err(|_| std::io::Error::other("lock"))?;
        if self.stopped() {
            let mut child = child;
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::other("monitor stopped"));
        }
        *player = Some(child);
        Ok(())
    }

    /// Writes monitor chunks to the player until it fails or the monitor stops.
    fn pump(&self) -> Pump {
        let Some(mut stdin) = self
            .player
            .lock()
            .ok()
            .and_then(|mut player| player.as_mut()?.stdin.take())
        else {
            return Pump::PlayerGone;
        };
        let mut bytes = Vec::new();
        loop {
            if self.stopped() {
                return Pump::PlayerGone;
            }
            match self.chunks.recv_timeout(CHUNK_WAIT) {
                Ok(chunk) => {
                    bytes.clear();
                    bytes.extend(
                        chunk
                            .samples()
                            .iter()
                            .flat_map(|sample| sample.to_le_bytes()),
                    );
                    if let Err(error) = stdin.write_all(&bytes) {
                        if !self.stopped() {
                            eprintln!("Bluetooth audio player stopped: {error}");
                        }
                        return Pump::PlayerGone;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Pump::Disconnected,
            }
        }
    }

    /// Drops monitor audio while waiting to restart the player. Returns false
    /// once the monitor is gone.
    fn discard_until(&self, deadline: Instant) -> bool {
        while !self.stopped() {
            let now = Instant::now();
            if now >= deadline {
                return true;
            }
            match self.chunks.recv_timeout(deadline - now) {
                Ok(_) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return false,
            }
        }
        false
    }

    fn stop_player(&self) {
        if let Ok(mut player) = self.player.lock() {
            if let Some(mut child) = player.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Pump {
    PlayerGone,
    Disconnected,
}

fn player_args(address: &str, sample_rate: u32) -> Vec<String> {
    vec![
        "-q".into(),
        "-t".into(),
        "raw".into(),
        "-f".into(),
        "S16_LE".into(),
        "-c".into(),
        "2".into(),
        "-r".into(),
        sample_rate.to_string(),
        format!("--buffer-time={PLAYER_BUFFER_MICROS}"),
        "-D".into(),
        format!("bluealsa:DEV={address},PROFILE=a2dp"),
    ]
}

/// The feeder and its player run as ordinary background work on the DSP
/// cores, below every audio thread.
fn lower_priority() {
    #[cfg(target_os = "linux")]
    unsafe {
        let param = libc::sched_param { sched_priority: 0 };
        libc::sched_setscheduler(0, libc::SCHED_OTHER, &param);
        libc::setpriority(libc::PRIO_PROCESS, 0, 10);
    }
    #[cfg(any(
        feature = "hardware-orange-pi-zero-2w",
        feature = "hardware-raspberry-pi-zero-2w"
    ))]
    crate::audio_priority::pin_background_worker_to_dsp_cpus();
}

#[cfg(test)]
mod tests;
