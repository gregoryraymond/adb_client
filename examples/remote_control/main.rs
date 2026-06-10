//! Minimal minitouch remote-control example.
//!
//! Usage: `remote_control <minitouch-prebuilt-root>`
//!
//! `<minitouch-prebuilt-root>` is a checkout of
//! <https://github.com/DeviceFarmer/minitouch-prebuilt> (containing `prebuilt/<abi>/bin/...`).
//! Requires a running ADB server with a single device connected. Taps the centre of the
//! screen, then swipes left-to-right across the middle.

use std::error::Error;
use std::fs::File;
use std::net::{Ipv4Addr, SocketAddrV4};

use adb_client::ADBDeviceExt;
use adb_client::server::ADBServer;

const REMOTE_DIR: &str = "/data/local/tmp";

fn main() -> Result<(), Box<dyn Error>> {
    let prebuilt_root = std::env::args()
        .nth(1)
        .ok_or("usage: remote_control <minitouch-prebuilt-root>")?;

    let mut server = ADBServer::new(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 5037));
    let mut device = server.get_device()?;

    // 1. Pick the right minitouch build (PIE support landed in SDK 16).
    let properties = device.device_properties()?;
    let abi = properties.abi.ok_or("device did not report an ABI")?;
    let sdk_level = properties
        .sdk_level
        .ok_or("device did not report an SDK level")?;
    let binary = if sdk_level >= 16 {
        "minitouch"
    } else {
        "minitouch-nopie"
    };
    let local = format!("{prebuilt_root}/prebuilt/{abi}/bin/{binary}");

    // 2. Push the binary.
    println!("pushing {local} -> {REMOTE_DIR}/minitouch");
    device.push(File::open(&local)?, format!("{REMOTE_DIR}/minitouch"))?;

    // 3. Start minitouch; the banner gives the touch coordinate space.
    let mut session = device.start_minitouch(REMOTE_DIR)?;
    let banner = *session.banner();
    println!("minitouch started: {banner:?}");

    let (mid_x, mid_y) = (banner.max_x / 2, banner.max_y / 2);
    let pressure = banner.max_pressure.max(1);
    let controller = session.controller();

    // 4a. Tap the centre.
    controller.tap(mid_x, mid_y, pressure)?;

    // 4b. Swipe left-to-right across the middle.
    let start_x = banner.max_x / 4;
    let end_x = banner.max_x * 3 / 4;
    controller.touch_down(0, start_x, mid_y, pressure)?;
    controller.commit()?;
    for step in 1..=10 {
        let x = start_x + (end_x - start_x) * step / 10;
        controller.touch_move(0, x, mid_y, pressure)?;
        controller.commit()?;
        controller.wait(16)?;
    }
    controller.touch_up(0)?;
    controller.commit()?;

    session.stop()?;
    println!("done");
    Ok(())
}
