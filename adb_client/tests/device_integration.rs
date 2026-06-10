//! Integration tests exercising the live ADB paths against a real device.
//!
//! These require a running ADB server (`127.0.0.1:5037`) with **exactly one** device connected,
//! so they are `#[ignore]`d by default. Run them explicitly with:
//!
//! ```sh
//! cargo test -p adb_client -- --ignored
//! ```
//!
//! They intentionally cover the read-mostly paths added across the STF work
//! (properties, display, packages, screenshot, forward listing, JDWP). The minicap/minitouch
//! streaming paths are not covered here as they require pushing external prebuilt binaries —
//! see the `screen_mirror` / `remote_control` examples instead.

use std::net::{Ipv4Addr, SocketAddrV4};

use adb_client::server::ADBServer;
use adb_client::server_device::ADBServerDevice;
use adb_client::{ADBDeviceExt, PackageDetails, PackageListType, UserFilter};

/// Connect to the local ADB server and return the single connected device.
fn device() -> ADBServerDevice {
    let mut server = ADBServer::new(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 5037));
    server
        .get_device()
        .expect("a single device should be connected to the local ADB server")
}

#[test]
#[ignore = "requires a connected device"]
fn reads_device_properties() {
    let mut device = device();
    let properties = device.device_properties().expect("device_properties");
    assert!(properties.sdk_level.is_some(), "expected an SDK level");
    assert!(properties.abi.is_some(), "expected an ABI");
}

#[test]
#[ignore = "requires a connected device"]
fn reads_a_single_property() {
    let mut device = device();
    let sdk = device
        .get_property("ro.build.version.sdk")
        .expect("get_property");
    assert!(sdk.is_some(), "expected ro.build.version.sdk to be set");
}

#[test]
#[ignore = "requires a connected device"]
fn reads_display_info() {
    let mut device = device();
    let info = device.display_info().expect("display_info");
    assert!(
        info.width > 0 && info.height > 0,
        "expected a non-empty display"
    );
}

#[test]
#[ignore = "requires a connected device"]
fn lists_packages_including_framework() {
    let mut device = device();
    let packages = device
        .list_packages(&PackageListType::AllNonApex(
            PackageDetails::Normal,
            UserFilter::NoUserSpecified,
        ))
        .expect("list_packages");
    assert!(
        packages.iter().any(|package| package == "android"),
        "expected the framework 'android' package to be present"
    );
}

#[test]
#[ignore = "requires a connected device"]
fn captures_a_png_screenshot() {
    let mut device = device();
    let png = device.screencap().expect("screencap");
    assert!(png.starts_with(b"\x89PNG"), "expected PNG magic bytes");
}

#[test]
#[ignore = "requires a connected device"]
fn lists_forward_rules() {
    // Should succeed (and typically be empty) on a fresh device.
    let mut device = device();
    device.list_forward().expect("list_forward");
}

#[test]
#[ignore = "requires a connected device"]
fn lists_jdwp_pids() {
    // May be empty if no debuggable apps are running; just assert it doesn't error.
    let mut device = device();
    device.list_jdwp().expect("list_jdwp");
}
