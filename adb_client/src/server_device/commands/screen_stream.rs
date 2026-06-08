use std::time::Duration;

use crate::{
    ADBDeviceExt, Result, RustADBError,
    server_device::ADBServerDevice,
    stream::{MinicapOptions, MinicapSession, MinicapStream, build_device, minicap_launch_command},
};

/// Number of times to retry connecting to the minicap socket while it starts up.
const MINICAP_CONNECT_ATTEMPTS: usize = 50;
/// Delay between connection attempts.
const MINICAP_CONNECT_DELAY: Duration = Duration::from_millis(100);

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
        let serial = self.identifier.clone();
        let server = self.transport.get_socketaddr();
        let command = minicap_launch_command(binary_dir, options);

        // minicap runs until killed, so launch it on its own connection in a background thread.
        let launch_serial = serial.clone();
        let launcher = std::thread::spawn(move || -> Result<()> {
            let mut device = build_device(launch_serial, server);
            device.shell_command(&command, None, None)?;
            Ok(())
        });

        // Connect to the abstract socket, retrying until minicap has bound it.
        let mut last_error = None;
        for _ in 0..MINICAP_CONNECT_ATTEMPTS {
            match self.open_local("localabstract:minicap") {
                Ok(stream) => {
                    let stream = MinicapStream::new(stream)?;
                    return Ok(MinicapSession::new(stream, serial, server, launcher));
                }
                Err(error) => {
                    last_error = Some(error);
                    std::thread::sleep(MINICAP_CONNECT_DELAY);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            RustADBError::ADBRequestFailed("minicap socket did not become available".to_string())
        }))
    }
}
