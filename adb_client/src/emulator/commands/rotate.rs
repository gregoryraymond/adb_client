use crate::{
    Result,
    emulator::{ADBEmulatorCommand, ADBEmulatorDevice},
};

impl ADBEmulatorDevice {
    /// Rotate this emulator's screen orientation.
    pub fn rotate(&mut self) -> Result<()> {
        let _ = self.connect()?.send_command(&ADBEmulatorCommand::Rotate)?;
        Ok(())
    }
}
