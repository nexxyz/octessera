use super::MAX_HEADER_BYTES;
use std::fs::File;
use std::io::{self, Read, Write};
use std::net::TcpStream;

pub(super) struct HttpRequest {
    pub(super) method: String,
    pub(super) path: String,
    pub(super) query: Option<String>,
    pub(super) headers: std::collections::BTreeMap<String, String>,
    pub(super) content_length: Option<u64>,
}

pub(super) fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut bytes = Vec::with_capacity(1024);
    let mut chunk = [0; 1];
    let header_end = loop {
        let read = stream.read(&mut chunk).map_err(io_error)?;
        if read == 0 {
            return Err("request ended before headers".into());
        }
        bytes.extend_from_slice(&chunk[..read]);
        if bytes.len() > MAX_HEADER_BYTES {
            return Err("request headers are too large".into());
        }
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let header_text = std::str::from_utf8(&bytes[..header_end]).map_err(|_| "invalid headers")?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or_else(|| "missing method".to_string())?;
    let target = parts.next().ok_or_else(|| "missing target".to_string())?;
    if parts.next() != Some("HTTP/1.1") {
        return Err("unsupported HTTP version".into());
    }
    let (path, query) = target
        .split_once('?')
        .map_or((target, None), |(path, query)| {
            (path, Some(query.to_string()))
        });
    let mut headers = std::collections::BTreeMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| "invalid header".to_string())?;
        let name = name.to_ascii_lowercase();
        if headers.contains_key(&name) {
            return Err("duplicate header".into());
        }
        headers.insert(name, value.trim().to_string());
    }
    let content_length = headers
        .get("content-length")
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| "invalid content length".to_string())
        })
        .transpose()?;
    if headers.contains_key("transfer-encoding") {
        return Err("chunked requests are not supported".into());
    }
    Ok(HttpRequest {
        method: method.into(),
        path: path.into(),
        query,
        headers,
        content_length,
    })
}

pub(super) fn copy_request_body(
    stream: &mut TcpStream,
    file: &mut File,
    mut length: u64,
) -> Result<(), String> {
    let mut buffer = [0; 16 * 1024];
    while length > 0 {
        let amount = (length as usize).min(buffer.len());
        stream.read_exact(&mut buffer[..amount]).map_err(io_error)?;
        file.write_all(&buffer[..amount]).map_err(io_error)?;
        length -= amount as u64;
    }
    Ok(())
}

pub(super) fn respond(stream: &mut TcpStream, status: u16, content_type: &str, body: &[u8]) {
    let header = response_header(status, content_type, body.len() as u64, None);
    let _ = stream.write_all(&header);
    let _ = stream.write_all(body);
}

pub(super) fn response_header(
    status: u16,
    content_type: &str,
    length: u64,
    filename: Option<&str>,
) -> Vec<u8> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        405 => "Method Not Allowed",
        409 => "Conflict",
        411 => "Length Required",
        413 => "Payload Too Large",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let disposition = filename
        .map(|filename| format!("Content-Disposition: attachment; filename=\"{filename}\"\r\n"))
        .unwrap_or_default();
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {length}\r\n{disposition}Connection: close\r\n\r\n"
    )
    .into_bytes()
}

fn io_error(error: io::Error) -> String {
    error.to_string()
}
