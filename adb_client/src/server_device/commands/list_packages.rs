use crate::{
    device::MessageWriter, models::{AdbServerCommand, PackageListType, SyncCommand}, ADBServerDevice, Result, RustADBError
};
use std::io::Write;
use std::io::Read;

impl ADBServerDevice {
    pub(crate) fn handle_list_packages(&mut self, package_list: &PackageListType) -> Result<()> {
        let mut len_buf = [0_u8; 4];
        // LittleEndian::write_u32(&mut len_buf, );

        // TODO: Main thing to fill out here

        // 4 bytes of command name is already sent by send_sync_request
        self.transport.get_raw_connection()?.write_all(&len_buf)?;
        self.transport
            .get_raw_connection()?
            .write_all(&[])?;

        // Reads returned status code from ADB server
        let mut response = [0_u8; 4];
        self.transport
            .get_raw_connection()?
            .read_exact(&mut response)?;
        match std::str::from_utf8(response.as_ref())? {
            "STAT" => {
                let mut data = [0_u8; 12];
                self.transport.get_raw_connection()?.read_exact(&mut data)?;

                Ok(())
            }
            x => Err(RustADBError::UnknownResponseType(format!(
                "Unknown response {}",
                x
            ))),
        }
    }
    pub(crate) fn list_packages(&mut self, package_list: &PackageListType) -> Result<()> {
        self.set_serial_transport()?;

        // Set device in SYNC mode
        self.transport.send_adb_request(AdbServerCommand::Sync)?;

        // Send a list command
        self.transport.send_sync_request(SyncCommand::List)?;

        self.handle_list_packages(package_list)
    }
}
