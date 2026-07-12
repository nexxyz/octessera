use crate::input::{grid_message, neokey_message};
use octessera_hal::{NeoKey, NeoTrellis, SeesawInterrupt};
use playback_runtime::HostMessage;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

const INPUT_SERVICE_INTERVAL: Duration = Duration::from_millis(4);

#[derive(Clone)]
pub(crate) enum SeesawCommand {
    GridFrame([[u8; 3]; 64]),
    NeoKeyColors([[u8; 3]; 4]),
}

pub(crate) struct SeesawIo {
    pub(crate) input_rx: Receiver<HostMessage>,
    pub(crate) command_tx: Sender<SeesawCommand>,
}

pub(crate) fn spawn(
    mut trellis: NeoTrellis,
    mut neokey: Option<NeoKey>,
    interrupt: SeesawInterrupt,
) -> SeesawIo {
    let (input_tx, input_rx) = mpsc::channel::<HostMessage>();
    let (command_tx, command_rx) = mpsc::channel::<SeesawCommand>();
    thread::spawn(move || {
        let mut previous_neokey = [false; 4];
        let mut previous_grid_frame: Option<[[u8; 3]; 64]> = None;
        let mut previous_neokey_colors: Option<[[u8; 3]; 4]> = None;
        let mut last_input_service = Instant::now() - INPUT_SERVICE_INTERVAL;
        loop {
            let service_due = last_input_service.elapsed() >= INPUT_SERVICE_INTERVAL;
            if service_due || interrupt.pending() {
                scan_inputs(
                    &mut trellis,
                    neokey.as_mut(),
                    &mut previous_neokey,
                    &input_tx,
                );
                last_input_service = Instant::now();
            }

            drain_commands(
                &command_rx,
                &mut trellis,
                neokey.as_mut(),
                &mut previous_grid_frame,
                &mut previous_neokey_colors,
            );

            thread::sleep(Duration::from_millis(2));
        }
    });

    SeesawIo {
        input_rx,
        command_tx,
    }
}

fn drain_commands(
    command_rx: &Receiver<SeesawCommand>,
    trellis: &mut NeoTrellis,
    neokey: Option<&mut NeoKey>,
    previous_grid_frame: &mut Option<[[u8; 3]; 64]>,
    previous_neokey_colors: &mut Option<[[u8; 3]; 4]>,
) {
    let mut latest_grid = None;
    let mut latest_neokey = None;
    for _ in 0..32 {
        let Ok(command) = command_rx.try_recv() else {
            break;
        };
        match command {
            SeesawCommand::GridFrame(frame) => latest_grid = Some(frame),
            SeesawCommand::NeoKeyColors(colors) => latest_neokey = Some(colors),
        }
    }

    if let Some(frame) = latest_grid {
        if previous_grid_frame.as_ref() != Some(&frame) && trellis.write_led_frame(&frame).is_ok() {
            *previous_grid_frame = Some(frame);
        }
    }

    if let (Some(colors), Some(neokey)) = (latest_neokey, neokey) {
        let previous = previous_neokey_colors.unwrap_or([[u8::MAX; 3]; 4]);
        let mut all_ok = true;
        for (index, color) in colors.iter().enumerate() {
            if previous.get(index) == Some(color) {
                continue;
            }
            all_ok &= neokey
                .set_led(index as u8, color[0], color[1], color[2])
                .is_ok();
        }
        if all_ok {
            *previous_neokey_colors = Some(colors);
        }
    }
}

fn scan_inputs(
    trellis: &mut NeoTrellis,
    neokey: Option<&mut NeoKey>,
    previous_neokey: &mut [bool; 4],
    input_tx: &Sender<HostMessage>,
) {
    if let Ok(presses) = trellis.scan_keys() {
        for (x, y, pressed) in presses {
            crate::wake_trace::log_trellis_event(x, y, pressed);
            let _ = input_tx.send(grid_message(x, y, pressed));
        }
    }

    if let Some(neokey) = neokey {
        let Ok(keys) = neokey.scan_interrupts() else {
            return;
        };
        for (key, pressed) in keys {
            let index = usize::from(key.min(3));
            if previous_neokey[index] == pressed {
                continue;
            }
            previous_neokey[index] = pressed;
            crate::wake_trace::log_neokey_transition(key, pressed);
            if let Some(message) = neokey_message(key, pressed) {
                let _ = input_tx.send(message);
            }
        }
    }
}
