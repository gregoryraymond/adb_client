use std::io::Read;

use crate::{
    Result,
    models::{ADBCommand, ADBLocalCommand, ForwardRule, parse_forward_list},
    server_device::ADBServerDevice,
};

impl ADBServerDevice {
    /// Forward socket connection
    pub fn forward(&mut self, remote: String, local: String) -> Result<()> {
        self.set_serial_transport()?;

        self.transport
            .proxy_connection(
                &ADBCommand::Local(ADBLocalCommand::Forward(remote, local)),
                false,
            )
            .map(|_| ())
    }

    /// Forward a dynamically-allocated local TCP port (`tcp:0`) to `remote`, returning the
    /// port the ADB server picked.
    ///
    /// Useful for connecting to a device-side socket such as `localabstract:minicap` without
    /// hard-coding a local port: the returned port can then be reached with a plain
    /// [`std::net::TcpStream`].
    pub fn forward_tcp(&mut self, remote: String) -> Result<u16> {
        self.set_serial_transport()?;

        // `host:forward` replies with *two* OKAYs (the smart-socket accept, then "forward
        // established"); for a `tcp:0` local spec the server then sends the allocated port as a
        // 4-hex-length-prefixed string. This matches AOSP adb's client, which does
        // adb_connect (OKAY) + adb_status (OKAY) + ReadProtocolString (the port).
        self.transport
            .send_adb_request(&ADBCommand::Local(ADBLocalCommand::Forward(
                remote,
                "tcp:0".to_string(),
            )))?; // first OKAY
        self.transport.read_adb_response()?; // second OKAY

        let length = self.transport.get_hex_body_length()? as usize;
        let mut port = vec![0u8; length];
        let mut connection = self.transport.get_raw_connection()?;
        connection.read_exact(&mut port)?;

        Ok(std::str::from_utf8(&port)?.trim().parse::<u16>()?)
    }

    /// List all forward rules known to the ADB server (`adb forward --list`).
    ///
    /// Note: this is a host-global query — it returns rules for *every* connected device, each
    /// tagged with its serial in [`ForwardRule::serial`], not just this device.
    pub fn list_forward(&mut self) -> Result<Vec<ForwardRule>> {
        let response = self
            .connect()?
            .proxy_connection(&ADBCommand::Local(ADBLocalCommand::ListForward), true)?;

        Ok(parse_forward_list(&String::from_utf8(response)?))
    }

    /// Remove a previously applied forward rule by its local endpoint.
    pub fn forward_remove(&mut self, local: String) -> Result<()> {
        self.set_serial_transport()?;

        self.transport
            .proxy_connection(
                &ADBCommand::Local(ADBLocalCommand::ForwardRemove(local)),
                false,
            )
            .map(|_| ())
    }

    /// Remove all previously applied forward rules
    pub fn forward_remove_all(&mut self) -> Result<()> {
        self.set_serial_transport()?;

        self.transport
            .proxy_connection(&ADBCommand::Local(ADBLocalCommand::ForwardRemoveAll), false)
            .map(|_| ())
    }
}
