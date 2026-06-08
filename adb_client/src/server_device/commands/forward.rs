use crate::{
    Result, RustADBError,
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

        let response = self.transport.proxy_connection(
            &ADBCommand::Local(ADBLocalCommand::Forward(remote, "tcp:0".to_string())),
            true,
        )?;

        std::str::from_utf8(&response)?
            .trim()
            .parse::<u16>()
            .map_err(RustADBError::from)
    }

    /// List all forward rules known to the ADB server (`adb forward --list`).
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
