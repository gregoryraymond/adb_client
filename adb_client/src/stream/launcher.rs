use std::io::Read;
use std::net::{SocketAddrV4, TcpStream};
use std::thread::JoinHandle;

use super::{MinicapHeader, MinicapStream};
use super::{Minitouch, MinitouchBanner};
use crate::{ADBDeviceExt, Result, RustADBError, server_device::ADBServerDevice};

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
    // Single-quote the (caller-supplied) directory so spaces or shell metacharacters in the
    // path cannot break or inject into the device-side shell command. The projection is
    // digits/`x@/` only and needs no quoting.
    format!(
        "LD_LIBRARY_PATH='{dir}' '{dir}/minicap' -P {projection}",
        dir = binary_dir,
        projection = options.projection()
    )
}

/// Build the shell command launching minitouch from `binary_dir`.
pub(crate) fn minitouch_launch_command(binary_dir: &str) -> String {
    // Single-quote the caller-supplied path (see `minicap_launch_command`).
    format!("'{binary_dir}/minitouch'")
}

/// Cleanup handle shared by the streaming sessions: kills a launched device-side process and
/// detaches its launcher thread.
#[derive(Debug)]
struct ProcessCleanup {
    serial: Option<String>,
    server: SocketAddrV4,
    pid: u32,
    launcher: JoinHandle<Result<()>>,
}

impl ProcessCleanup {
    /// Shut down the cleanup handle held in `slot` (if any). Shared by both sessions' `stop`
    /// and `Drop`.
    fn take_and_shutdown(slot: &mut Option<Self>) -> Result<()> {
        slot.take().map_or(Ok(()), Self::shutdown)
    }

    fn shutdown(self) -> Result<()> {
        let Self {
            serial,
            server,
            pid,
            launcher,
        } = self;
        // Kill the device-side process; the launcher's blocking shell_command then returns as
        // it observes the exit.
        let mut device = build_device(serial, server);
        let result = device
            .shell_command(&format!("kill {pid}"), None, None)
            .map(|_| ());
        // Detach the launcher rather than join()-ing it: the thread winds down on its own once
        // the kill lands, and a missed kill must never hang the caller (or block Drop forever).
        drop(launcher);
        result
    }
}

/// A running minicap session: a launched minicap process plus the connected frame stream.
///
/// Dropping the session makes a best-effort attempt to kill the device-side minicap process;
/// call [`MinicapSession::stop`] to do so explicitly and surface any error.
#[derive(Debug)]
pub struct MinicapSession {
    stream: MinicapStream<TcpStream>,
    cleanup: Option<ProcessCleanup>,
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
            cleanup: Some(ProcessCleanup {
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

    /// Read the next JPEG frame (blocking). `Ok(None)` signals a clean end of stream.
    pub fn next_frame(&mut self) -> Result<Option<Vec<u8>>> {
        self.stream.next_frame()
    }

    /// Stop the session: kill the device-side minicap process.
    pub fn stop(mut self) -> Result<()> {
        ProcessCleanup::take_and_shutdown(&mut self.cleanup)
    }
}

impl Drop for MinicapSession {
    fn drop(&mut self) {
        let _ = ProcessCleanup::take_and_shutdown(&mut self.cleanup);
    }
}

/// A running minitouch session: a launched minitouch process plus a connected controller.
///
/// Dropping the session makes a best-effort attempt to kill the device-side process; call
/// [`MinitouchSession::stop`] to do so explicitly and surface any error.
#[derive(Debug)]
pub struct MinitouchSession {
    controller: Minitouch<TcpStream>,
    banner: MinitouchBanner,
    cleanup: Option<ProcessCleanup>,
}

impl MinitouchSession {
    pub(crate) fn new(
        controller: Minitouch<TcpStream>,
        banner: MinitouchBanner,
        serial: Option<String>,
        server: SocketAddrV4,
        launcher: JoinHandle<Result<()>>,
    ) -> Self {
        Self {
            controller,
            banner,
            cleanup: Some(ProcessCleanup {
                serial,
                server,
                pid: banner.pid,
                launcher,
            }),
        }
    }

    /// The minitouch banner describing its limits (max contacts / x / y / pressure).
    #[must_use]
    pub fn banner(&self) -> &MinitouchBanner {
        &self.banner
    }

    /// Mutable access to the controller for sending touch commands.
    pub fn controller(&mut self) -> &mut Minitouch<TcpStream> {
        &mut self.controller
    }

    /// Stop the session: kill the device-side minitouch process.
    pub fn stop(mut self) -> Result<()> {
        ProcessCleanup::take_and_shutdown(&mut self.cleanup)
    }
}

impl Drop for MinitouchSession {
    fn drop(&mut self) {
        let _ = ProcessCleanup::take_and_shutdown(&mut self.cleanup);
    }
}

/// Read the minitouch startup banner from `stream`, consuming bytes up to and including the
/// `$ <pid>` line.
///
/// Reads one byte at a time rather than via a `BufReader`, so it does not consume past the
/// banner into the shared socket buffer (which the controller half then writes to).
pub(crate) fn read_minitouch_banner(stream: &TcpStream) -> Result<MinitouchBanner> {
    let mut reader = stream.try_clone()?;
    let mut text = String::new();
    let mut byte = [0u8; 1];
    // The banner is three lines (v / ^ / $); stop after the `$` line, and cap the total read so
    // a misbehaving binary cannot make us spin forever.
    let mut at_line_start = true;
    let mut done = false;
    while !done && text.len() < 1024 {
        if reader.read(&mut byte).map_err(RustADBError::IOError)? == 0 {
            break;
        }
        let c = byte[0];
        // The `$ <pid>` line is the last one; stop once we've consumed its trailing newline.
        let pid_line = at_line_start && c == b'$';
        at_line_start = c == b'\n';
        text.push(c as char);
        if pid_line {
            // Consume the rest of this line up to and including the newline.
            while reader.read(&mut byte).map_err(RustADBError::IOError)? != 0 {
                text.push(byte[0] as char);
                if byte[0] == b'\n' {
                    break;
                }
            }
            done = true;
        }
    }
    MinitouchBanner::parse(&text)
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
            "LD_LIBRARY_PATH='/data/local/tmp' '/data/local/tmp/minicap' -P 1080x2340@1080x2340/0"
        );
    }
}
