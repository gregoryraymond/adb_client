use std::net::{SocketAddrV4, TcpStream};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::{
    ADBDeviceExt, Result, RustADBError,
    server_device::ADBServerDevice,
    stream::{
        MinicapOptions, MinicapSession, MinicapStream, Minitouch, MinitouchSession, build_device,
        minicap_launch_command, minitouch_launch_command, read_minitouch_banner,
    },
};

/// Number of times to retry connecting to a streaming socket while the process starts up.
const CONNECT_ATTEMPTS: usize = 50;
/// Delay between connection attempts.
const CONNECT_DELAY: Duration = Duration::from_millis(100);

/// A launched device-side process and the stream connected to its socket.
struct LaunchedProcess {
    stream: TcpStream,
    serial: Option<String>,
    server: SocketAddrV4,
    launcher: JoinHandle<Result<()>>,
}

impl ADBServerDevice {
    /// Launch minicap on the device and return a connected [`MinicapSession`] yielding frames.
    ///
    /// `binary_dir` is the device-side directory holding the `minicap` executable and the
    /// matching `minicap.so` (e.g. `/data/local/tmp`); push the correct ABI build there first
    /// (see [`crate::ADBDeviceExt::device_properties`] to choose it). minicap is launched on a
    /// dedicated connection in a background thread, then this method connects to
    /// `localabstract:minicap`, retrying while the process binds the socket.
    ///
    /// The returned session kills the device-side process when stopped or dropped.
    pub fn start_minicap(
        &mut self,
        binary_dir: &str,
        options: &MinicapOptions,
    ) -> Result<MinicapSession> {
        let command = minicap_launch_command(binary_dir, options);
        let process = self.launch_and_connect(command, "localabstract:minicap")?;

        let stream = MinicapStream::new(process.stream)?;
        Ok(MinicapSession::new(
            stream,
            process.serial,
            process.server,
            process.launcher,
        ))
    }

    /// Launch minitouch on the device and return a connected [`MinitouchSession`] for injecting
    /// touch events.
    ///
    /// `binary_dir` is the device-side directory holding the `minitouch` executable. The startup
    /// banner is read and parsed, exposing the coordinate/pressure limits via
    /// [`MinitouchSession::banner`]. The returned session kills the device-side process when
    /// stopped or dropped.
    pub fn start_minitouch(&mut self, binary_dir: &str) -> Result<MinitouchSession> {
        let command = minitouch_launch_command(binary_dir);
        let process = self.launch_and_connect(command, "localabstract:minitouch")?;

        let banner = read_minitouch_banner(&process.stream)?;
        let controller = Minitouch::new(process.stream);
        Ok(MinitouchSession::new(
            controller,
            banner,
            process.serial,
            process.server,
            process.launcher,
        ))
    }

    /// Launch `command` on a dedicated background connection, then connect to `service`,
    /// retrying while the process binds it. Returns the opened stream plus the bits needed to
    /// later kill the launched process.
    fn launch_and_connect(&mut self, command: String, service: &str) -> Result<LaunchedProcess> {
        let serial = self.identifier.clone();
        let server = self.transport.get_socketaddr();

        // The binary runs until killed, so launch it on its own connection in a background thread.
        let launch_serial = serial.clone();
        let launcher = std::thread::spawn(move || -> Result<()> {
            let mut device = build_device(launch_serial, server);
            device.shell_command(&command, None, None)?;
            Ok(())
        });

        let mut last_error = None;
        for _ in 0..CONNECT_ATTEMPTS {
            match self.open_local(service) {
                Ok(stream) => {
                    return Ok(LaunchedProcess {
                        stream,
                        serial,
                        server,
                        launcher,
                    });
                }
                Err(error) => {
                    last_error = Some(error);
                    std::thread::sleep(CONNECT_DELAY);
                }
            }
        }

        // The socket never came up. Best-effort kill the process we launched so the background
        // thread — blocked in its shell_command — can wind down instead of leaking its
        // connection, then detach it and surface the error. The binary name is the service's
        // suffix (e.g. `localabstract:minicap` -> `minicap`).
        let binary = service.rsplit(':').next().unwrap_or(service);
        let mut cleanup = build_device(serial, server);
        let _ = cleanup.shell_command(&format!("pkill -f {binary}"), None, None);
        drop(launcher);

        Err(last_error.unwrap_or_else(|| {
            RustADBError::ADBRequestFailed(format!("{service} socket did not become available"))
        }))
    }
}
