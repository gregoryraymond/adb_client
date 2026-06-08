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
//! [`MinicapAssets::resolve`] maps a device ABI + SDK level to the asset paths within a
//! `minicap-prebuilt` checkout to push.
//!
//! ## Rotation
//!
//! minicap captures a fixed projection, so a display rotation requires a restart: stop the
//! current [`MinicapSession`] and call [`crate::server_device::ADBServerDevice::start_minicap`]
//! again with [`MinicapOptions`] built for the new orientation. Detecting the rotation itself
//! (e.g. via the on-device agent or polling) is left to the caller.
//!
//! [minicap]: https://github.com/DeviceFarmer/minicap
//! [minitouch]: https://github.com/DeviceFarmer/minitouch
//! [minicap-prebuilt]: https://github.com/DeviceFarmer/minicap-prebuilt

mod launcher;
mod minicap;
mod minitouch;

pub use launcher::{MinicapAssets, MinicapOptions, MinicapSession, MinitouchSession};
pub(crate) use launcher::{
    build_device, minicap_launch_command, minitouch_launch_command, read_minitouch_banner,
};
pub use minicap::{MinicapHeader, MinicapStream};
pub use minitouch::{Minitouch, MinitouchBanner};
