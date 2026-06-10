//! Minimal minicap screen-mirroring example.
//!
//! Usage: `screen_mirror <minicap-prebuilt-root> [frame_count]`
//!
//! `<minicap-prebuilt-root>` is a checkout of
//! <https://github.com/DeviceFarmer/minicap-prebuilt> (containing `prebuilt/<abi>/...`).
//! Requires a running ADB server with a single device connected. Saves the first frames
//! as `frame_NNNN.jpg` in the working directory.

use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddrV4};

use adb_client::ADBDeviceExt;
use adb_client::server::ADBServer;
use adb_client::server_device::ADBServerDevice;
use adb_client::stream::{MinicapAssets, MinicapOptions};

const REMOTE_DIR: &str = "/data/local/tmp";

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let prebuilt_root = args
        .next()
        .ok_or("usage: screen_mirror <minicap-prebuilt-root> [frame_count]")?;
    let frame_count: usize = args.next().and_then(|v| v.parse().ok()).unwrap_or(10);

    let mut server = ADBServer::new(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 5037));
    let mut device = server.get_device()?;

    // 1. Pick the right minicap build for this device's ABI + SDK level.
    let properties = device.device_properties()?;
    let abi = properties.abi.ok_or("device did not report an ABI")?;
    let sdk_level = properties
        .sdk_level
        .ok_or("device did not report an SDK level")?;
    let assets = MinicapAssets::resolve(&abi, sdk_level)?;
    println!("device abi={abi} sdk={sdk_level}");

    // 2. Push the executable and its shared library to the device.
    push(
        &mut device,
        &format!("{prebuilt_root}/{}", assets.binary),
        &format!("{REMOTE_DIR}/minicap"),
    )?;
    push(
        &mut device,
        &format!("{prebuilt_root}/{}", assets.library),
        &format!("{REMOTE_DIR}/minicap.so"),
    )?;

    // 3. Build the projection from the current display and start streaming.
    let display = device.display_info()?;
    let options = MinicapOptions::from_display_info(&display, 0);
    let mut session = device.start_minicap(REMOTE_DIR, &options)?;
    println!("minicap started: {:?}", session.header());

    // 4. Save the first frames as JPEGs (stop early if the stream ends).
    for index in 0..frame_count {
        let Some(frame) = session.next_frame()? else {
            println!("minicap stream ended");
            break;
        };
        let path = format!("frame_{index:04}.jpg");
        File::create(&path)?.write_all(&frame)?;
        println!("wrote {path} ({} bytes)", frame.len());
    }

    session.stop()?;
    Ok(())
}

fn push(device: &mut ADBServerDevice, local: &str, remote: &str) -> Result<(), Box<dyn Error>> {
    println!("pushing {local} -> {remote}");
    device.push(File::open(local)?, remote)?;
    Ok(())
}
