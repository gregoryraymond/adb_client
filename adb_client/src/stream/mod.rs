//! Screen streaming and remote input over [minicap] and [minitouch].
//!
//! This module provides the wire-protocol codecs for the two binaries STF uses for live
//! screen mirroring and remote control. It does **not** ship or launch the binaries — that
//! orchestration (selecting the right ABI build, `push`-ing it, and launching it with `exec`)
//! is left to the caller. The typical flow is:
//!
//! 1. Pick the binary for the device ABI (see [`crate::ADBDeviceExt::device_properties`]),
//!    `push` it to `/data/local/tmp`, and launch it with a shell/`exec` session.
//! 2. Open its abstract socket with
//!    [`crate::server_device::ADBServerDevice::open_local`] (`localabstract:minicap` /
//!    `localabstract:minitouch`).
//! 3. Drive it with [`MinicapStream`] (read JPEG frames) / [`Minitouch`] (write touch events).
//!
//! [minicap]: https://github.com/DeviceFarmer/minicap
//! [minitouch]: https://github.com/DeviceFarmer/minitouch

mod launcher;
mod minicap;
mod minitouch;

pub use launcher::{MinicapOptions, MinicapSession};
pub(crate) use launcher::{build_device, minicap_launch_command};
pub use minicap::{MinicapHeader, MinicapStream};
pub use minitouch::{Minitouch, MinitouchBanner};
