use std::net::{SocketAddrV4, TcpStream};
use std::thread::JoinHandle;

use super::{MinicapHeader, MinicapStream};
use crate::{ADBDeviceExt, Result, server_device::ADBServerDevice};

/// Display projection passed to minicap's `-P` flag: the real (physical) size, the virtual
/// (output) size frames are scaled to, and the rotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinicapOptions {
    /// Real (physical) display width.
    pub real_width: u32,
    /// Real (physical) display height.
    pub real_height: u32,
    /// Virtual (output) frame width.
    pub virtual_width: u32,
    /// Virtual (output) frame height.
    pub virtual_height: u32,
    /// Rotation in 90° steps (`0..=3`).
    pub orientation: u8,
}

impl MinicapOptions {
    /// Project at the native resolution (real == virtual), upright.
    #[must_use]
    pub fn native(width: u32, height: u32) -> Self {
        Self {
            real_width: width,
            real_height: height,
            virtual_width: width,
            virtual_height: height,
            orientation: 0,
        }
    }

    /// The `-P` projection argument: `<rw>x<rh>@<vw>x<vh>/<rotation>`.
    #[must_use]
    pub fn projection(&self) -> String {
        format!(
            "{}x{}@{}x{}/{}",
            self.real_width,
            self.real_height,
            self.virtual_width,
            self.virtual_height,
            self.orientation
        )
    }
}

/// Build the shell command launching minicap from `binary_dir` (which must contain both the
/// `minicap` executable and the matching `minicap.so`).
pub(crate) fn minicap_launch_command(binary_dir: &str, options: &MinicapOptions) -> String {
    format!(
        "LD_LIBRARY_PATH={dir} {dir}/minicap -P {projection}",
        dir = binary_dir,
        projection = options.projection()
    )
}

/// A running minicap session: a launched minicap process plus the connected frame stream.
///
/// Dropping the session makes a best-effort attempt to kill the device-side minicap process;
/// call [`MinicapSession::stop`] to do so explicitly and surface any error.
#[derive(Debug)]
pub struct MinicapSession {
    stream: MinicapStream<TcpStream>,
    cleanup: Option<MinicapCleanup>,
}

#[derive(Debug)]
struct MinicapCleanup {
    serial: Option<String>,
    server: SocketAddrV4,
    pid: u32,
    launcher: JoinHandle<Result<()>>,
}

impl MinicapCleanup {
    fn shutdown(self) -> Result<()> {
        // Kill the device-side process; the launcher's blocking shell_command then returns.
        let mut device = build_device(self.serial, self.server);
        let result = device
            .shell_command(&format!("kill {}", self.pid), None, None)
            .map(|_| ());
        let _ = self.launcher.join();
        result
    }
}

impl MinicapSession {
    pub(crate) fn new(
        stream: MinicapStream<TcpStream>,
        serial: Option<String>,
        server: SocketAddrV4,
        launcher: JoinHandle<Result<()>>,
    ) -> Self {
        let pid = stream.header().pid;
        Self {
            stream,
            cleanup: Some(MinicapCleanup {
                serial,
                server,
                pid,
                launcher,
            }),
        }
    }

    /// The minicap banner describing the stream geometry.
    #[must_use]
    pub fn header(&self) -> &MinicapHeader {
        self.stream.header()
    }

    /// Read the next JPEG frame (blocking).
    pub fn next_frame(&mut self) -> Result<Vec<u8>> {
        self.stream.next_frame()
    }

    /// Stop the session: kill the device-side minicap process and wait for the launcher.
    pub fn stop(mut self) -> Result<()> {
        match self.cleanup.take() {
            Some(cleanup) => cleanup.shutdown(),
            None => Ok(()),
        }
    }
}

impl Drop for MinicapSession {
    fn drop(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            let _ = cleanup.shutdown();
        }
    }
}

/// Construct a device for a one-off control command on a fresh connection.
pub(crate) fn build_device(serial: Option<String>, server: SocketAddrV4) -> ADBServerDevice {
    match serial {
        Some(serial) => ADBServerDevice::new(serial, Some(server)),
        None => ADBServerDevice::autodetect(Some(server)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_format() {
        let options = MinicapOptions {
            real_width: 1080,
            real_height: 2340,
            virtual_width: 720,
            virtual_height: 1560,
            orientation: 1,
        };
        assert_eq!(options.projection(), "1080x2340@720x1560/1");
    }

    #[test]
    fn native_projection_is_identity() {
        assert_eq!(
            MinicapOptions::native(1080, 2340).projection(),
            "1080x2340@1080x2340/0"
        );
    }

    #[test]
    fn launch_command_sets_library_path() {
        let command =
            minicap_launch_command("/data/local/tmp", &MinicapOptions::native(1080, 2340));
        assert_eq!(
            command,
            "LD_LIBRARY_PATH=/data/local/tmp /data/local/tmp/minicap -P 1080x2340@1080x2340/0"
        );
    }
}
