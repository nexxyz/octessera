use std::time::Duration;

pub(crate) trait SeesawTransport: Send {
    fn write(&mut self, base: u8, function: u8, data: &[u8]) -> Result<(), String>;
    fn read(&mut self, base: u8, function: u8, buffer: &mut [u8]) -> Result<(), String>;
}

pub(crate) trait SeesawDelay: Send {
    fn sleep(&mut self, duration: Duration);
}

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w"))]
pub(crate) struct ThreadSeesawDelay;

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w"))]
impl SeesawDelay for ThreadSeesawDelay {
    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

#[cfg(test)]
pub(crate) struct NoopSeesawDelay;

#[cfg(test)]
impl SeesawDelay for NoopSeesawDelay {
    fn sleep(&mut self, _duration: Duration) {}
}

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w"))]
pub(crate) struct LinuxSeesawTransport {
    file: std::fs::File,
    address: u16,
    device: &'static str,
}

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w"))]
impl LinuxSeesawTransport {
    pub(crate) fn new(i2c_path: &str, address: u16, device: &'static str) -> Result<Self, String> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(i2c_path)
            .map_err(|error| format!("{device} I2C open failed at {address:#04x}: {error}"))?;
        let transport = Self {
            file,
            address,
            device,
        };
        transport.select_address()?;
        Ok(transport)
    }

    fn select_address(&self) -> Result<(), String> {
        set_slave_address(&self.file, self.address).map_err(|error| {
            format!(
                "{} I2C slave select failed at {:#04x}: {error}",
                self.device, self.address
            )
        })
    }

    fn register_error(
        &self,
        operation: &str,
        base: u8,
        function: u8,
        error: std::io::Error,
    ) -> String {
        format!(
            "{} I2C {operation} failed at {:#04x} register {base:#04x}/{function:#04x}: {error}",
            self.device, self.address
        )
    }
}

#[cfg(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w"))]
impl SeesawTransport for LinuxSeesawTransport {
    fn write(&mut self, base: u8, function: u8, data: &[u8]) -> Result<(), String> {
        use std::io::Write;

        let mut command = Vec::with_capacity(2 + data.len());
        command.extend_from_slice(&[base, function]);
        command.extend_from_slice(data);
        self.file
            .write_all(&command)
            .map_err(|error| self.register_error("write", base, function, error))
    }

    fn read(&mut self, base: u8, function: u8, buffer: &mut [u8]) -> Result<(), String> {
        use std::io::{Read, Write};

        self.file
            .write_all(&[base, function])
            .map_err(|error| self.register_error("read command write", base, function, error))?;
        std::thread::sleep(Duration::from_millis(1));
        self.file
            .read_exact(buffer)
            .map_err(|error| self.register_error("read", base, function, error))
    }
}

#[cfg(feature = "orange-pi-zero-2w")]
#[cfg(not(unix))]
fn set_slave_address(_file: &std::fs::File, _address: u16) -> Result<(), String> {
    Err("Orange Seesaw I2C requires a Unix target".into())
}

#[cfg(any(
    feature = "raspberry-pi-zero-2w",
    all(feature = "orange-pi-zero-2w", unix)
))]
fn set_slave_address(file: &std::fs::File, address: u16) -> Result<(), String> {
    #[cfg(all(unix, target_os = "linux"))]
    {
        use std::os::unix::io::AsRawFd;

        let result = unsafe { libc::ioctl(file.as_raw_fd(), 0x0703, address as u64) };
        if result < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    let _ = (file, address);
    Ok(())
}
