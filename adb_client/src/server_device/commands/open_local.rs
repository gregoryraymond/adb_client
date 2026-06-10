use std::net::TcpStream;

use crate::{
    Result,
    models::{ADBCommand, ADBLocalCommand},
    server_device::ADBServerDevice,
};

impl ADBServerDevice {
    /// Open a raw bidirectional stream to a device-side local service through the ADB server.
    ///
    /// `service` is an ADB local service string such as `localabstract:minicap`,
    /// `localreserved:<name>`, `localfilesystem:<path>`, `tcp:<port>` or `jdwp:<pid>`. This is
    /// the primitive used to talk to on-device sockets like `minicap` (read JPEG frames) and
    /// `minitouch` (write touch events) without setting up a port forward.
    ///
    /// The returned [`TcpStream`] is fully owned by the caller: it is moved out of the device's
    /// transport, so subsequent commands on this [`ADBServerDevice`] reconnect on a fresh
    /// connection and will not shut the returned stream down. Call
    /// [`TcpStream::try_clone`](std::net::TcpStream::try_clone) to obtain independent read and
    /// write halves for concurrent use.
    pub fn open_local(&mut self, service: &str) -> Result<TcpStream> {
        self.set_serial_transport()?;

        self.transport
            .send_adb_request(&ADBCommand::Local(ADBLocalCommand::Open(
                service.to_string(),
            )))?;

        self.transport.take_connection()
    }
}
