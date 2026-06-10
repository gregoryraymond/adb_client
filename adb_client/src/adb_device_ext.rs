use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;

#[cfg(feature = "framebuffer")]
use {
    image::{ImageBuffer, ImageFormat, Rgba},
    std::io::Cursor,
};

use crate::models::{
    ADBListItemType, AdbStatResponse, DeviceProperties, DisplayInfo, PackageListType, RemountInfo,
    UserFilter, escape_input_text, parse_getprop, parse_wm_density, parse_wm_size,
};
use crate::{ADBStatExtendedResponse, RebootType, Result, RustADBError};

/// Trait representing all features available on ADB devices.
pub trait ADBDeviceExt {
    /// Runs command in a shell on the device, and write its output and error streams into output.
    fn shell_command(
        &mut self,
        command: &dyn AsRef<str>,
        stdout: Option<&mut dyn Write>,
        stderr: Option<&mut dyn Write>,
    ) -> Result<Option<u8>>;

    /// Starts an interactive shell session on the device.
    /// Input data is read from reader and write to writer.
    fn shell(&mut self, reader: &mut dyn Read, writer: Box<dyn Write + Send>) -> Result<()>;

    /// Runs command on the device.
    /// Input data is read from reader and write to writer.
    fn exec(
        &mut self,
        command: &str,
        reader: &mut dyn Read,
        writer: Box<dyn Write + Send>,
    ) -> Result<()>;

    /// Display the stat information for a remote file using STAT protocol command.
    fn stat(&mut self, remote_path: &dyn AsRef<str>) -> Result<AdbStatResponse>;

    /// Display the stat information for a remote file using `stat` shell command.
    /// This is an extended version of `stat` that returns more detailed information.
    /// Returns `Ok(None)` if the file does not exist on the device.
    fn stat_extended(
        &mut self,
        remote_path: &dyn AsRef<str>,
    ) -> Result<Option<ADBStatExtendedResponse>> {
        let mut stdout = Vec::new();
        self.shell_command(
            &format!("stat {}", remote_path.as_ref()),
            Some(&mut stdout),
            None,
        )?;

        // all parsing magic happens here...
        ADBStatExtendedResponse::try_from(&stdout)
    }

    /// Pull the remote file pointed to by `source` and write its contents into `output`
    fn pull(&mut self, source: &dyn AsRef<str>, output: &mut dyn Write) -> Result<()>;

    /// Push `stream` to `path` on the device.
    fn push(&mut self, stream: &mut dyn Read, path: &dyn AsRef<str>) -> Result<()>;

    /// List the items in a directory on the device
    fn list(&mut self, path: &dyn AsRef<str>) -> Result<Vec<ADBListItemType>>;

    /// Reboot the device using given reboot type
    fn reboot(&mut self, reboot_type: RebootType) -> Result<()>;

    /// Remount the device partitions as read-write
    fn remount(&mut self) -> Result<Vec<RemountInfo>>;

    /// Restart adb daemon with root permissions
    fn root(&mut self) -> Result<()>;

    /// Run `activity` from `package` on device. Return the command output.
    fn run_activity(
        &mut self,
        package: &dyn AsRef<str>,
        activity: &dyn AsRef<str>,
    ) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        let _status = self.shell_command(
            &format!(
                "am start {}/{}.{}",
                package.as_ref(),
                package.as_ref(),
                activity.as_ref()
            ),
            Some(&mut output),
            None,
        )?;

        Ok(output)
    }

    /// Install an APK pointed to by `apk_path` on device.
    fn install(&mut self, apk_path: &dyn AsRef<Path>, user: Option<&str>) -> Result<()>;

    /// Uninstall the package `package` from device.
    fn uninstall(&mut self, package: &dyn AsRef<str>, user: Option<&str>) -> Result<()>;

    /// List packages installed on the device, returning their identifiers.
    ///
    /// This wraps the device's `pm list packages` command. `package_filter` selects which
    /// set of packages to return, how much detail to include for each entry, and which user
    /// to query. Each returned [`String`] is the matching `pm` output line with the leading
    /// `package:` marker stripped, so depending on the requested [`crate::PackageDetails`] it
    /// may also carry the APK path, version code or installer.
    fn list_packages(&mut self, package_filter: &PackageListType) -> Result<Vec<String>> {
        let (filter_flag, details, user_filter) = package_filter.components();

        let mut command = format!("pm list packages {filter_flag}");

        if let Some(detail_flag) = details.flag() {
            command.push(' ');
            command.push_str(detail_flag);
        }

        let user_id = match user_filter {
            UserFilter::NoUserSpecified => None,
            UserFilter::SpecificUser(user_id) => Some(*user_id),
            UserFilter::CurrentUser => {
                let mut current_user = Vec::new();
                self.shell_command(
                    &"cmd activity get-current-user",
                    Some(&mut current_user),
                    None,
                )?;
                Some(String::from_utf8(current_user)?.trim().parse::<u32>()?)
            }
        };

        if let Some(user_id) = user_id {
            command.push_str(" --user ");
            command.push_str(&user_id.to_string());
        }

        let mut output = Vec::new();
        self.shell_command(&command, Some(&mut output), None)?;

        Ok(String::from_utf8(output)?
            .lines()
            .filter_map(|line| line.strip_prefix("package:"))
            .map(|package| package.trim().to_string())
            .filter(|package| !package.is_empty())
            .collect())
    }

    /// Return all device properties, as reported by `getprop`, as a key/value map.
    fn get_properties(&mut self) -> Result<HashMap<String, String>> {
        let mut output = Vec::new();
        self.shell_command(&"getprop", Some(&mut output), None)?;
        Ok(parse_getprop(&String::from_utf8(output)?))
    }

    /// Return the value of a single device property (`getprop <name>`), or `None` if unset.
    ///
    /// `name` must be a valid Android property key (`[A-Za-z0-9._-]`); other characters are
    /// rejected so the value cannot alter the shell command (e.g. inject a default argument or
    /// extra tokens). Only the trailing line ending is stripped from the value.
    fn get_property(&mut self, name: &str) -> Result<Option<String>> {
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        {
            return Err(RustADBError::ADBRequestFailed(format!(
                "invalid property name: {name:?}"
            )));
        }

        let mut output = Vec::new();
        self.shell_command(&format!("getprop {name}"), Some(&mut output), None)?;
        let value = String::from_utf8(output)?;
        let value = value.trim_end_matches(['\r', '\n']);
        Ok((!value.is_empty()).then(|| value.to_string()))
    }

    /// Return commonly-used device properties (model, ABI, SDK level, ...) as typed fields.
    fn device_properties(&mut self) -> Result<DeviceProperties> {
        Ok(DeviceProperties::from_property_map(&self.get_properties()?))
    }

    /// Return the physical display geometry, as reported by `wm size` / `wm density`.
    fn display_info(&mut self) -> Result<DisplayInfo> {
        let mut size_output = Vec::new();
        self.shell_command(&"wm size", Some(&mut size_output), None)?;
        let (width, height) =
            parse_wm_size(&String::from_utf8(size_output)?).ok_or(RustADBError::ConversionError)?;

        let mut density_output = Vec::new();
        self.shell_command(&"wm density", Some(&mut density_output), None)?;
        let density = parse_wm_density(&String::from_utf8(density_output)?);

        Ok(DisplayInfo {
            width,
            height,
            density,
        })
    }

    /// Inject a key event (`input keyevent <keycode>`).
    ///
    /// Accepts a raw Android key code; [`crate::KeyCode`] provides constants for common keys,
    /// e.g. `device.input_keyevent(KeyCode::Home.into())`.
    fn input_keyevent(&mut self, keycode: i32) -> Result<()> {
        self.shell_command(&format!("input keyevent {keycode}"), None, None)?;
        Ok(())
    }

    /// Type text on the device (`input text`). Spaces and shell-special characters are escaped.
    fn input_text(&mut self, text: &str) -> Result<()> {
        self.shell_command(
            &format!("input text {}", escape_input_text(text)),
            None,
            None,
        )?;
        Ok(())
    }

    /// Tap the screen at `(x, y)` (`input tap`).
    fn input_tap(&mut self, x: u32, y: u32) -> Result<()> {
        self.shell_command(&format!("input tap {x} {y}"), None, None)?;
        Ok(())
    }

    /// Swipe from `(x1, y1)` to `(x2, y2)` over `duration_ms` milliseconds (`input swipe`).
    fn input_swipe(&mut self, x1: u32, y1: u32, x2: u32, y2: u32, duration_ms: u32) -> Result<()> {
        self.shell_command(
            &format!("input swipe {x1} {y1} {x2} {y2} {duration_ms}"),
            None,
            None,
        )?;
        Ok(())
    }

    /// Enable dm-verity on the device
    fn enable_verity(&mut self) -> Result<()>;

    /// Disable dm-verity on the device
    fn disable_verity(&mut self) -> Result<()>;

    #[cfg(feature = "framebuffer")]
    /// Inner method requesting framebuffer from an Android device
    fn framebuffer_inner(&mut self) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>>;

    /// Dump framebuffer of this device into given path.
    ///
    /// Output data format is currently only `PNG`.
    #[cfg(feature = "framebuffer")]
    fn framebuffer(&mut self, path: &dyn AsRef<Path>) -> Result<()> {
        // Big help from AOSP source code (<https://android.googlesource.com/platform/system/adb/+/refs/heads/main/framebuffer_service.cpp>)
        let img = self.framebuffer_inner()?;
        Ok(img.save(path.as_ref())?)
    }

    /// Dump framebuffer of this device and return corresponding bytes.
    ///
    /// Output data format is currently only `PNG`.
    #[cfg(feature = "framebuffer")]
    fn framebuffer_bytes(&mut self) -> Result<Vec<u8>> {
        let img = self.framebuffer_inner()?;
        let mut vec = Cursor::new(Vec::new());
        img.write_to(&mut vec, ImageFormat::Png)?;

        Ok(vec.into_inner())
    }

    /// Return a boxed instance representing this trait
    fn boxed(self) -> Box<dyn ADBDeviceExt>
    where
        Self: Sized + 'static,
    {
        Box::new(self)
    }
}
