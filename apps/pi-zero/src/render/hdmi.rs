use serde_json::Value;

#[cfg(unix)]
mod imp {
    use super::*;
    use std::fs::OpenOptions;
    use std::io::{Seek, SeekFrom, Write};

    pub struct HdmiFramebuffer {
        file: Option<std::fs::File>,
        path: String,
        width: usize,
        height: usize,
        bytes_per_pixel: usize,
    }

    impl HdmiFramebuffer {
        pub fn open_from_env() -> Option<Self> {
            if std::env::var("OCTESSERA_HDMI_DISABLE").ok().as_deref() == Some("1") {
                return None;
            }
            let bytes_per_pixel = match std::env::var("OCTESSERA_HDMI_FB_BPP").ok().as_deref() {
                Some("16") => 2,
                _ => 4,
            };
            Some(Self {
                file: None,
                path: std::env::var("OCTESSERA_HDMI_FB").unwrap_or_else(|_| "/dev/fb0".into()),
                width: 640,
                height: 480,
                bytes_per_pixel,
            })
        }

        pub fn render(&mut self, snapshot: &Value) {
            if hdmi_mode(snapshot) == Some("none") {
                self.blank_and_close();
                return;
            }
            let Some(frame) =
                compose_frame(snapshot, self.width, self.height, self.bytes_per_pixel)
            else {
                return;
            };
            if self.file.is_none() {
                self.file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&self.path)
                    .ok();
            }
            let Some(file) = self.file.as_mut() else {
                return;
            };
            let _ = file.seek(SeekFrom::Start(0));
            let _ = file.write_all(&frame);
        }

        fn blank_and_close(&mut self) {
            if let Some(mut file) = self.file.take() {
                let frame = vec![0_u8; self.width * self.height * self.bytes_per_pixel];
                let _ = file.seek(SeekFrom::Start(0));
                let _ = file.write_all(&frame);
            }
        }
    }
}

#[cfg(not(unix))]
mod imp {
    use super::*;
    pub struct HdmiFramebuffer;
    impl HdmiFramebuffer {
        pub fn open_from_env() -> Option<Self> {
            None
        }
        pub fn render(&mut self, snapshot: &Value) {
            let _ = compose_frame(snapshot, 1, 1, 4);
        }
    }
}

pub use imp::HdmiFramebuffer;

pub fn compose_frame(
    snapshot: &Value,
    width: usize,
    height: usize,
    bytes_per_pixel: usize,
) -> Option<Vec<u8>> {
    if hdmi_mode(snapshot) == Some("none") {
        return None;
    }
    let grid = snapshot.get("hdmi").and_then(|hdmi| hdmi.get("grid"))?;
    let rgb = grid.get("rgb").and_then(Value::as_array)?;
    let show_gridlines = snapshot
        .get("hdmi")
        .and_then(|hdmi| hdmi.get("showGridlines"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let side = width.min(height);
    let cell = side / 8;
    if cell == 0 || (bytes_per_pixel != 2 && bytes_per_pixel != 4) {
        return None;
    }
    let square = cell * 8;
    let x0 = (width - square) / 2;
    let y0 = (height - square) / 2;
    let mut frame = vec![0_u8; width * height * bytes_per_pixel];
    for gy in 0..8 {
        for gx in 0..8 {
            let index = gy * 8 + gx;
            let color = [
                u8_at(rgb, index * 3),
                u8_at(rgb, index * 3 + 1),
                u8_at(rgb, index * 3 + 2),
            ];
            for py in 0..cell {
                for px in 0..cell {
                    if show_gridlines && (px == 0 || py == 0) {
                        continue;
                    }
                    let offset =
                        ((y0 + gy * cell + py) * width + x0 + gx * cell + px) * bytes_per_pixel;
                    write_pixel(
                        &mut frame[offset..offset + bytes_per_pixel],
                        color,
                        bytes_per_pixel,
                    );
                }
            }
        }
    }
    Some(frame)
}

fn hdmi_mode(snapshot: &Value) -> Option<&str> {
    snapshot.get("hdmi")?.get("mode")?.as_str()
}

fn u8_at(values: &[Value], index: usize) -> u8 {
    values
        .get(index)
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(255) as u8
}

fn write_pixel(pixel: &mut [u8], color: [u8; 3], bytes_per_pixel: usize) {
    if bytes_per_pixel == 2 {
        let value = (u16::from(color[0] >> 3) << 11)
            | (u16::from(color[1] >> 2) << 5)
            | u16::from(color[2] >> 3);
        pixel.copy_from_slice(&value.to_ne_bytes());
    } else {
        pixel.copy_from_slice(&[color[2], color[1], color[0], 0]);
    }
}

pub fn hdmi_signature(snapshot: &Value) -> u64 {
    if hdmi_mode(snapshot) == Some("none") {
        return 0;
    }
    let bytes =
        serde_json::to_vec(snapshot.get("hdmi").unwrap_or(&Value::Null)).unwrap_or_default();
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn none_mode_has_no_signature_or_frame() {
        let snapshot = json!({
            "hdmi": {
                "mode": "none",
                "grid": {
                    "width": 8,
                    "height": 8,
                    "rgb": vec![255; 8 * 8 * 3],
                    "active": vec![true; 8 * 8]
                }
            }
        });

        assert_eq!(hdmi_signature(&snapshot), 0);
        assert!(compose_frame(&snapshot, 64, 64, 4).is_none());
    }
}
