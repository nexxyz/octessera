use super::*;
use crate::oled_frame_cache::OledFramePublication;
use octessera_hal::OledSsd1351;
use playback_runtime::oled_frame::OLED_FRAME_BYTES;
use serde_json::json;
use std::io;
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

struct BlockingHdmiIo {
    entered: mpsc::Sender<()>,
    release: Arc<(Mutex<bool>, Condvar)>,
}

impl crate::render::hdmi::device::HdmiIo for BlockingHdmiIo {
    fn open_framebuffer(
        &mut self,
        path: &str,
    ) -> Result<
        Box<dyn crate::render::hdmi::device::FramebufferHandle>,
        crate::render::hdmi::HdmiError,
    > {
        self.entered.send(()).unwrap();
        let (lock, ready) = &*self.release;
        let mut released = lock.lock().unwrap();
        while !*released {
            released = ready.wait(released).unwrap();
        }
        Err(crate::render::hdmi::HdmiError::Io {
            operation: "open",
            path: path.into(),
            source: io::Error::from_raw_os_error(libc::EIO),
        })
    }

    fn open_tty(
        &mut self,
        path: &str,
    ) -> Result<Box<dyn crate::render::hdmi::device::TtyHandle>, crate::render::hdmi::HdmiError>
    {
        Err(crate::render::hdmi::HdmiError::Io {
            operation: "open",
            path: path.into(),
            source: io::Error::from_raw_os_error(libc::EIO),
        })
    }
}

fn native_snapshot() -> serde_json::Value {
    json!({
        "display": { "off": false },
        "settings": { "buttonBrightness": 100, "displayBrightness": 100 },
        "leds": { "rgb": vec![0; 64 * 3] },
        "transport": { "playing": false },
        "transportIcon": "stop",
        "transportFlash": "none",
        "eventDotOn": false,
        "oledFrameRevision": 1,
        "neoKeyLeds": {
            "back": [0, 0, 0],
            "space": [0, 0, 0],
            "shift": [0, 0, 0],
            "fn": [0, 0, 0]
        }
    })
}

#[test]
fn initial_ack_and_handoff_complete_before_blocking_hdmi_failure_and_retry() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let hdmi_device = crate::render::hdmi::device::HdmiDevice::new(
        Box::new(BlockingHdmiIo {
            entered: entered_tx,
            release: Arc::clone(&release),
        }),
        "/dev/fb0",
        "/dev/tty1",
    );
    let (seesaw_tx, _seesaw_rx) = mpsc::channel();
    let worker = RenderWorker::spawn(crate::render::HardwareRenderTargets {
        oled: OledSsd1351::new().unwrap(),
        seesaw_tx,
        oled_handoff: None,
        hdmi: crate::render::hdmi::HdmiFramebuffer::from_device(hdmi_device),
    });
    let (result_tx, result_rx) = mpsc::channel();
    let worker_for_publish = worker.clone();
    let publish = thread::spawn(move || {
        let result = worker_for_publish.publish_acknowledged_snapshot(
            native_snapshot(),
            OledFramePublication::test_native(1, vec![0; OLED_FRAME_BYTES]),
        );
        result_tx.send(result).unwrap();
    });

    entered_rx
        .try_recv()
        .expect_err("HDMI attempt began before the initial OLED acknowledgement");
    let oled_ack = result_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("initial OLED acknowledgement did not complete");
    assert_eq!(oled_ack, Ok(()));
    publish.join().unwrap();

    let (handoff_result_tx, handoff_result_rx) = mpsc::channel();
    let worker_for_handoff = worker.clone();
    let handoff = thread::spawn(move || {
        handoff_result_tx
            .send(worker_for_handoff.mark_first_menu_rendered())
            .unwrap();
    });
    let handoff_result = handoff_result_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("first-menu handoff acknowledgement did not complete");
    assert_eq!(handoff_result, Ok(()));
    handoff.join().unwrap();

    entered_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("HDMI worker did not reach its blocking open after handoff");
    {
        let (lock, ready) = &*release;
        *lock.lock().unwrap() = true;
        ready.notify_one();
    }

    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("HDMI retry did not retain its retry deadline");
    worker.abort().unwrap();
}
