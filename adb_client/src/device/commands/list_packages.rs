use crate::{
    device::{adb_message_device::ADBMessageDevice, MessageWriter}, models::PackageListType, ADBMessageTransport, Result
};

impl<T: ADBMessageTransport> ADBMessageDevice<T> {
    pub(crate) fn list_packages(&mut self, package_list: &PackageListType) -> Result<()> {

        //TODO: Heres the client side of list packages
        self.open_session(format!("exec:cmd package 'install'\0").as_bytes())?;
        let transport = self.get_transport().clone();

        let mut writer = MessageWriter::new(transport, 0, self.get_remote_id()?);

        let final_status = self.get_transport_mut().read_message()?;

        match final_status.into_payload().as_slice() {
            b"Success\n" => {
                log::info!(
                    "APK file successfully installed",
                );
                Ok(())
            }
            d => Err(crate::RustADBError::ADBRequestFailed(String::from_utf8(
                d.to_vec(),
            )?)),
        }
    }
}
