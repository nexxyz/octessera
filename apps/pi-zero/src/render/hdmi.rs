use serde_json::Value;
use std::fmt;
use std::io;

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Debug)]
pub enum HdmiError {
    Io {
        operation: &'static str,
        path: String,
        source: io::Error,
    },
    InvalidGeometry {
        path: String,
        width: u32,
        height: u32,
        stride: u32,
        bits_per_pixel: u32,
    },
    UnsupportedFormat {
        path: String,
        bits_per_pixel: u32,
        red: (u32, u32),
        green: (u32, u32),
        blue: (u32, u32),
        transp: (u32, u32),
    },
}

impl fmt::Display for HdmiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "{operation} framebuffer {path}: {source}"),
            Self::InvalidGeometry {
                path,
                width,
                height,
                stride,
                bits_per_pixel,
            } => write!(
                formatter,
                "invalid framebuffer geometry for {path}: {width}x{height}, stride {stride}, {bits_per_pixel} bpp"
            ),
            Self::UnsupportedFormat {
                path,
                bits_per_pixel,
                red,
                green,
                blue,
                transp,
            } => write!(
                formatter,
                "unsupported framebuffer format for {path}: {bits_per_pixel} bpp, red {red:?}, green {green:?}, blue {blue:?}, transparency {transp:?}"
            ),
        }
    }
}

impl std::error::Error for HdmiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::InvalidGeometry { .. } | Self::UnsupportedFormat { .. } => None,
        }
    }
}

#[cfg(target_os = "linux")]
#[path = "hdmi_linux.rs"]
mod imp;

#[cfg(not(target_os = "linux"))]
mod imp {
    use super::*;

    pub struct HdmiFramebuffer;

    impl HdmiFramebuffer {
        pub fn open_from_env() -> Result<Option<Self>, HdmiError> {
            Ok(None)
        }

        pub fn render(&mut self, snapshot: &Value) -> Result<(), HdmiError> {
            let _ = compose_frame(snapshot, 1, 1, 4);
            Ok(())
        }
    }
}

pub use imp::HdmiFramebuffer;

#[cfg(any(test, not(target_os = "linux")))]
pub fn compose_frame(
    snapshot: &Value,
    width: usize,
    height: usize,
    bytes_per_pixel: usize,
) -> Option<Vec<u8>> {
    let stride = width.checked_mul(bytes_per_pixel)?;
    compose_frame_with_stride(snapshot, width, height, stride, bytes_per_pixel)
}

pub fn compose_frame_with_stride(
    snapshot: &Value,
    width: usize,
    height: usize,
    stride: usize,
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
    let minimum_stride = width.checked_mul(bytes_per_pixel)?;
    if stride < minimum_stride {
        return None;
    }
    let side = width.min(height);
    let cell = side / 8;
    if cell == 0 || (bytes_per_pixel != 2 && bytes_per_pixel != 4) {
        return None;
    }
    let square = cell * 8;
    let x0 = (width - square) / 2;
    let y0 = (height - square) / 2;
    let mut frame = vec![0_u8; stride.checked_mul(height)?];
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
                        (y0 + gy * cell + py) * stride + (x0 + gx * cell + px) * bytes_per_pixel;
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

    #[test]
    fn none_mode_has_no_signature_or_frame() {
        let snapshot = serde_json::json!({
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

    #[test]
    fn stride_preserves_snapshot_row_order_and_padding() {
        let mut rgb = vec![0; 8 * 8 * 3];
        rgb[0..3].copy_from_slice(&[255, 0, 0]);
        let bottom_left = 7 * 8 * 3;
        rgb[bottom_left..bottom_left + 3].copy_from_slice(&[0, 0, 255]);
        let snapshot = serde_json::json!({
            "hdmi": {
                "mode": "live-grid",
                "grid": { "rgb": rgb },
                "showGridlines": false
            }
        });

        let frame = compose_frame_with_stride(&snapshot, 16, 8, 72, 4).unwrap();

        assert_eq!(&frame[4 * 4..4 * 4 + 4], &[0, 0, 255, 0]);
        assert_eq!(&frame[7 * 72 + 4 * 4..7 * 72 + 4 * 4 + 4], &[255, 0, 0, 0]);
        assert!(frame[16 * 4..72].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn stride_and_pixel_format_are_required() {
        let snapshot = serde_json::json!({
            "hdmi": {
                "mode": "live-grid",
                "grid": { "rgb": vec![255; 8 * 8 * 3] }
            }
        });

        assert!(compose_frame_with_stride(&snapshot, 16, 8, 63, 4).is_none());
        assert!(compose_frame_with_stride(&snapshot, 16, 8, 64, 3).is_none());
    }
}
