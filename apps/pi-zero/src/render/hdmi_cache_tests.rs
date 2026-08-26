use super::*;
use serde_json::{json, Value};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct CacheHdmiControl {
    fail_text: usize,
    events: Vec<&'static str>,
}

struct CacheHdmiIo {
    control: Arc<Mutex<CacheHdmiControl>>,
}

struct CacheHdmiFramebuffer {
    control: Arc<Mutex<CacheHdmiControl>>,
}

struct CacheHdmiTty {
    control: Arc<Mutex<CacheHdmiControl>>,
}

impl hdmi::device::HdmiIo for CacheHdmiIo {
    fn open_framebuffer(
        &mut self,
        _path: &str,
    ) -> Result<Box<dyn hdmi::device::FramebufferHandle>, hdmi::HdmiError> {
        self.control.lock().unwrap().events.push("open_fb");
        Ok(Box::new(CacheHdmiFramebuffer {
            control: Arc::clone(&self.control),
        }))
    }

    fn open_tty(
        &mut self,
        _path: &str,
    ) -> Result<Box<dyn hdmi::device::TtyHandle>, hdmi::HdmiError> {
        self.control.lock().unwrap().events.push("open_tty");
        Ok(Box::new(CacheHdmiTty {
            control: Arc::clone(&self.control),
        }))
    }
}

impl hdmi::device::FramebufferHandle for CacheHdmiFramebuffer {
    fn geometry(&self) -> hdmi::device::FramebufferGeometry {
        hdmi::device::FramebufferGeometry {
            width: 64,
            height: 64,
            stride: 64 * 4,
            bytes_per_pixel: 4,
        }
    }

    fn unblank(&mut self) -> io::Result<()> {
        self.control.lock().unwrap().events.push("unblank");
        Ok(())
    }

    fn write_frame(&mut self, _frame: &[u8]) -> io::Result<()> {
        self.control.lock().unwrap().events.push("write");
        Ok(())
    }
}

impl hdmi::device::TtyHandle for CacheHdmiTty {
    fn set_graphics(&mut self) -> io::Result<()> {
        self.control.lock().unwrap().events.push("graphics");
        Ok(())
    }

    fn set_text(&mut self) -> io::Result<()> {
        let mut control = self.control.lock().unwrap();
        control.events.push("text");
        if control.fail_text > 0 {
            control.fail_text -= 1;
            Err(io::Error::from_raw_os_error(libc::EIO))
        } else {
            Ok(())
        }
    }
}

fn hdmi_snapshot(color: [u8; 3], mode: &str) -> Value {
    json!({
        "hdmi": {
            "mode": mode,
            "grid": { "rgb": color.repeat(64) },
            "showGridlines": false
        }
    })
}

#[test]
fn pending_hdmi_restore_survives_reversion_to_accepted_signature() {
    let control = Arc::new(Mutex::new(CacheHdmiControl::default()));
    let device = hdmi::device::HdmiDevice::new(
        Box::new(CacheHdmiIo {
            control: Arc::clone(&control),
        }),
        "/dev/fb0",
        "/dev/tty1",
    );
    let mut hdmi = hdmi::HdmiFramebuffer::from_device(device);
    let accepted = hdmi_snapshot([255, 0, 0], "live-grid");
    let terminal = hdmi_snapshot([0, 0, 0], "none");
    let mut cache = HardwareRenderCache::default();
    let start = Instant::now();

    assert!(render_hdmi_if_changed(&mut hdmi, &accepted, &mut cache, start).is_none());
    let accepted_signature = hdmi::hdmi_signature(&accepted);
    assert_eq!(cache.hdmi_signature, accepted_signature);

    control.lock().unwrap().fail_text = 1;
    let retry_at = render_hdmi_if_changed(
        &mut hdmi,
        &terminal,
        &mut cache,
        start + Duration::from_millis(1),
    )
    .unwrap();
    assert_eq!(cache.hdmi_signature, accepted_signature);

    let events_before_pending_call = control.lock().unwrap().events.len();
    assert_eq!(
        render_hdmi_if_changed(
            &mut hdmi,
            &accepted,
            &mut cache,
            retry_at - Duration::from_millis(1),
        ),
        Some(retry_at)
    );
    assert_eq!(
        control.lock().unwrap().events.len(),
        events_before_pending_call
    );

    assert!(render_hdmi_if_changed(&mut hdmi, &accepted, &mut cache, retry_at).is_none());
    assert_eq!(cache.hdmi_signature, accepted_signature);
    assert_eq!(
        control
            .lock()
            .unwrap()
            .events
            .iter()
            .filter(|event| **event == "text")
            .count(),
        2
    );
}
