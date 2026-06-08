use clap::Parser;

use super::DeviceCommands;

#[derive(Parser, Debug)]
pub enum LocalCommand {
    #[clap(flatten)]
    DeviceCommands(DeviceCommands),
    #[clap(flatten)]
    LocalDeviceCommand(LocalDeviceCommand),
}

#[derive(Parser, Debug)]
pub enum LocalDeviceCommand {
    /// List available server features.
    HostFeatures,
    /// Get logs of device
    Logcat {
        /// Path to output file (created if not exists)
        path: Option<String>,
    },
    /// Forward a local port to a device port.
    #[clap(subcommand)]
    Forward(ForwardCommand),
    /// Reverse a device port to a local port.
    #[clap(subcommand)]
    Reverse(ReverseCommand),
}

#[derive(Parser, Debug)]
pub enum ForwardCommand {
    /// Remove all forwarded ports.
    RemoveAll,
    /// Remove a specific forwarded port.
    Remove { local: String },
    /// Forward a local port to a device port.
    Add { local: String, remote: String },
    /// Forward a dynamically-allocated local TCP port to a device port, printing the port.
    Tcp { remote: String },
    /// List all forward rules.
    List,
}

#[derive(Parser, Debug)]
pub enum ReverseCommand {
    /// Remove all reversed ports.
    RemoveAll,
    /// Remove a specific reversed port, identified by its remote (device-side) endpoint.
    Remove { remote: String },
    /// Reverse a device port to a local port.
    Add { remote: String, local: String },
    /// List reverse rules for this device.
    List,
}
