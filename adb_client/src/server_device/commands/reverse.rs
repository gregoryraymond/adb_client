use crate::{
    Result,
    models::{ADBCommand, ADBLocalCommand, ForwardRule, parse_reverse_list},
    server_device::ADBServerDevice,
};

impl ADBServerDevice {
    /// Reverse socket connection
    pub fn reverse(&mut self, remote: String, local: String) -> Result<()> {
        self.set_serial_transport()?;

        self.transport
            .proxy_connection(
                &ADBCommand::Local(ADBLocalCommand::Reverse(remote, local)),
                false,
            )
            .map(|_| ())
    }

    /// Remove a previously applied reverse rule by its remote endpoint.
    pub fn reverse_remove(&mut self, remote: String) -> Result<()> {
        self.set_serial_transport()?;

        self.transport
            .proxy_connection(
                &ADBCommand::Local(ADBLocalCommand::ReverseRemove(remote)),
                false,
            )
            .map(|_| ())
    }

    /// Remove all reverse rules
    pub fn reverse_remove_all(&mut self) -> Result<()> {
        self.set_serial_transport()?;

        self.transport
            .proxy_connection(&ADBCommand::Local(ADBLocalCommand::ReverseRemoveAll), false)
            .map(|_| ())
    }

    /// List reverse rules for this device (`adb reverse --list`).
    pub fn list_reverse(&mut self) -> Result<Vec<ForwardRule>> {
        self.set_serial_transport()?;

        let response = self
            .transport
            .proxy_connection(&ADBCommand::Local(ADBLocalCommand::ListReverse), true)?;

        Ok(parse_reverse_list(&String::from_utf8(response)?))
    }
}
