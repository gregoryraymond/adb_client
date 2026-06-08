use std::path::PathBuf;

use adb_client::{PackageDetails, PackageListType, UserFilter};
use clap::{Parser, ValueEnum};

use super::RebootTypeCommand;

#[derive(Parser, Debug)]
pub enum DeviceCommands {
    /// Spawn an interactive shell or run a list of commands on the device
    Shell {
        #[arg(trailing_var_arg = true)]
        commands: Vec<String>,
    },
    /// Pull a file from device
    Pull { source: String, destination: String },
    /// Push a file on device
    Push { filename: String, path: String },
    /// Stat a file on device
    Stat { path: String },
    /// Stat a file on device with extended information
    StatExtended { path: String },
    /// Run an activity on device specified by the intent
    Run {
        /// The package whose activity is to be invoked
        #[clap(short = 'p', long = "package")]
        package: String,
        /// The activity to be invoked itself, Usually it is `MainActivity`
        #[clap(short = 'a', long = "activity")]
        activity: String,
    },
    /// Reboot the device
    Reboot {
        #[clap(subcommand)]
        reboot_type: RebootTypeCommand,
    },
    /// Install an APK on device
    Install {
        /// User id to install the package for
        #[clap(short = 'u', long = "user")]
        user: Option<String>,
        /// Path to APK file. Extension must be ".apk"
        path: PathBuf,
    },
    /// Uninstall a package from the device
    Uninstall {
        /// User id of the package to uninstall
        #[clap(short = 'u', long = "user")]
        user: Option<String>,
        /// Name of the package to uninstall
        package: String,
    },
    /// Dump framebuffer of device
    Framebuffer {
        /// Framebuffer image destination path
        path: String,
    },
    /// List files on device
    List {
        /// Path to list files from
        path: String,
    },
    /// List packages installed on device
    ListPackages {
        /// Which set of packages to list
        #[clap(long = "filter", value_enum, default_value_t = PackageFilter::All)]
        filter: PackageFilter,
        /// Additional detail to display for each package
        #[clap(long = "detail", value_enum, default_value_t = PackageDetail::Normal)]
        detail: PackageDetail,
        /// Restrict listing to the given user id
        #[clap(short = 'u', long = "user")]
        user: Option<u32>,
        /// Restrict listing to the device's current user (resolved on device)
        #[clap(long = "current-user", conflicts_with = "user")]
        current_user: bool,
    },
    /// Restart adb daemon with root permissions
    Root,
}

/// Which set of packages [`DeviceCommands::ListPackages`] should return.
#[derive(ValueEnum, Clone, Debug)]
pub enum PackageFilter {
    /// All packages, excluding APEX containers
    All,
    /// Only APEX packages
    Apex,
    /// Only disabled packages
    Disabled,
    /// Only enabled packages
    Enabled,
    /// Only system packages
    System,
    /// Only uninstalled packages whose data is kept
    Uninstalled,
}

/// Additional detail displayed for each listed package.
#[derive(ValueEnum, Clone, Debug)]
pub enum PackageDetail {
    /// Just the package identifier
    Normal,
    /// Also include the version code
    VersionCode,
    /// Also include the installer
    Installer,
    /// Also include the associated APK path
    Apks,
}

impl From<PackageDetail> for PackageDetails {
    fn from(value: PackageDetail) -> Self {
        match value {
            PackageDetail::Normal => PackageDetails::Normal,
            PackageDetail::VersionCode => PackageDetails::ShowVersionCode,
            PackageDetail::Installer => PackageDetails::ShowInstaller,
            PackageDetail::Apks => PackageDetails::ShowAssociatedApks,
        }
    }
}

impl PackageFilter {
    /// Build the [`PackageListType`] requested on the command line.
    pub fn into_package_list_type(
        self,
        detail: PackageDetail,
        user: UserFilter,
    ) -> PackageListType {
        let detail = detail.into();
        match self {
            PackageFilter::All => PackageListType::AllNonApex(detail, user),
            PackageFilter::Apex => PackageListType::Apex(detail, user),
            PackageFilter::Disabled => PackageListType::Disabled(detail, user),
            PackageFilter::Enabled => PackageListType::Enabled(detail, user),
            PackageFilter::System => PackageListType::System(detail, user),
            PackageFilter::Uninstalled => PackageListType::Uninstalled(detail, user),
        }
    }
}
