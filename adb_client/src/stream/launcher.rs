use std::io::Read;
use std::net::{SocketAddrV4, TcpStream};
use std::thread::JoinHandle;

use super::{MinicapHeader, MinicapStream};
use super::{Minitouch, MinitouchBanner};
use crate::{ADBDeviceExt, DisplayInfo, Result, RustADBError, server_device::ADBServerDevice};

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

    /// Project at the native resolution with the given rotation (`0..=3`, in 90° steps).
    #[must_use]
    pub fn with_orientation(width: u32, height: u32, orientation: u8) -> Self {
        Self {
            orientation,
            ..Self::native(width, height)
        }
    }

    /// Build options from a [`DisplayInfo`] (see [`crate::ADBDeviceExt::display_info`]) and a
    /// rotation. When the display rotates, rebuild with the new `orientation` and restart the
    /// session (see the module docs).
    #[must_use]
    pub fn from_display_info(display: &DisplayInfo, orientation: u8) -> Self {
        Self::with_orientation(display.width, display.height, orientation)
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

/// Relative paths to the minicap assets to push to the device, within a
/// [`minicap-prebuilt`](https://github.com/DeviceFarmer/minicap-prebuilt) checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinicapAssets {
    /// Path to the `minicap` executable (`minicap-nopie` on SDK < 16).
    pub binary: String,
    /// Path to the matching `minicap.so` for the device's SDK level.
    pub library: String,
}

impl MinicapAssets {
    /// Resolve the asset paths for a device ABI (e.g. `arm64-v8a`) and SDK level, following the
    /// `minicap-prebuilt` directory layout. PIE support was added in SDK 16, so older devices
    /// use the `minicap-nopie` build.
    ///
    /// The ABI is validated against `[A-Za-z0-9._-]` and rejected otherwise: it typically comes
    /// from a device-reported property (`ro.product.cpu.abi`), so an unvalidated value (e.g.
    /// containing `/` or `..`) could traverse outside the prebuilt directory when the caller
    /// joins these relative paths onto a base.
    pub fn resolve(abi: &str, sdk_level: u32) -> Result<Self> {
        if abi.is_empty()
            || !abi
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        {
            return Err(RustADBError::ADBRequestFailed(format!(
                "invalid device ABI: {abi:?}"
            )));
        }

        let binary_name = if sdk_level >= 16 {
            "minicap"
        } else {
            "minicap-nopie"
        };
        Ok(Self {
            binary: format!("prebuilt/{abi}/bin/{binary_name}"),
            library: format!("prebuilt/{abi}/lib/android-{sdk_level}/minicap.so"),
        })
    }
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

    #[test]
    fn options_with_orientation() {
        assert_eq!(
            MinicapOptions::with_orientation(1080, 2340, 1).projection(),
            "1080x2340@1080x2340/1"
        );
    }

    #[test]
    fn options_from_display_info() {
        let display = DisplayInfo {
            width: 720,
            height: 1280,
            density: Some(320),
        };
        let options = MinicapOptions::from_display_info(&display, 3);
        assert_eq!(options.real_width, 720);
        assert_eq!(options.real_height, 1280);
        assert_eq!(options.orientation, 3);
    }

    #[test]
    fn resolves_assets() {
        let assets = MinicapAssets::resolve("arm64-v8a", 34).unwrap();
        assert_eq!(assets.binary, "prebuilt/arm64-v8a/bin/minicap");
        assert_eq!(
            assets.library,
            "prebuilt/arm64-v8a/lib/android-34/minicap.so"
        );
    }

    #[test]
    fn resolves_nopie_for_old_sdk() {
        let assets = MinicapAssets::resolve("armeabi-v7a", 15).unwrap();
        assert_eq!(assets.binary, "prebuilt/armeabi-v7a/bin/minicap-nopie");
    }

    #[test]
    fn rejects_abi_with_path_traversal() {
        assert!(MinicapAssets::resolve("../../etc", 34).is_err());
        assert!(MinicapAssets::resolve("a/b", 34).is_err());
        assert!(MinicapAssets::resolve("", 34).is_err());
    }
}
