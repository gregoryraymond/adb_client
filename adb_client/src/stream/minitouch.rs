use std::io::Write;

use crate::{Result, RustADBError};

/// The startup banner emitted by `minitouch`, describing its limits.
///
/// See the minitouch protocol: <https://github.com/DeviceFarmer/minitouch#usage>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinitouchBanner {
    /// Protocol version (the `v <version>` line).
    pub version: u32,
    /// Maximum number of simultaneous contacts.
    pub max_contacts: u32,
    /// Maximum X coordinate.
    pub max_x: u32,
    /// Maximum Y coordinate.
    pub max_y: u32,
    /// Maximum pressure value.
    pub max_pressure: u32,
    /// PID of the minitouch process (the `$ <pid>` line).
    pub pid: u32,
}

impl MinitouchBanner {
    /// Parse the banner from the text minitouch prints on startup. Lines look like:
    ///
    /// ```text
    /// v 1
    /// ^ 10 4095 4095 255
    /// $ 12345
    /// ```
    pub fn parse(text: &str) -> Result<Self> {
        let mut version = None;
        let mut limits = None;
        let mut pid = None;

        for line in text.lines() {
            let mut tokens = line.split_whitespace();
            match tokens.next() {
                Some("v") => version = tokens.next().and_then(|v| v.parse().ok()),
                Some("^") => {
                    let values: Vec<u32> = tokens.filter_map(|token| token.parse().ok()).collect();
                    if let [max_contacts, max_x, max_y, max_pressure] = values[..] {
                        limits = Some((max_contacts, max_x, max_y, max_pressure));
                    }
                }
                Some("$") => pid = tokens.next().and_then(|v| v.parse().ok()),
                _ => {}
            }
        }

        let version = version.ok_or(RustADBError::ConversionError)?;
        let (max_contacts, max_x, max_y, max_pressure) =
            limits.ok_or(RustADBError::ConversionError)?;
        let pid = pid.ok_or(RustADBError::ConversionError)?;

        Ok(Self {
            version,
            max_contacts,
            max_x,
            max_y,
            max_pressure,
            pid,
        })
    }
}

/// Writer for the `minitouch` command protocol.
///
/// Wrap the stream returned by [`crate::server_device::ADBServerDevice::open_local`] with
/// `localabstract:minitouch` (after reading and parsing the banner with
/// [`MinitouchBanner::parse`]). Coordinates are in minitouch's own space (`0..=max_x`,
/// `0..=max_y`), so scale from display pixels using the banner limits.
#[derive(Debug)]
pub struct Minitouch<W: Write> {
    writer: W,
}

impl<W: Write> Minitouch<W> {
    /// Wrap a writable stream.
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Touch down: contact `id` at `(x, y)` with `pressure`. (`d <id> <x> <y> <pressure>`)
    pub fn touch_down(&mut self, id: u32, x: u32, y: u32, pressure: u32) -> Result<()> {
        self.command(format_args!("d {id} {x} {y} {pressure}\n"))
    }

    /// Move contact `id` to `(x, y)` with `pressure`. (`m <id> <x> <y> <pressure>`)
    pub fn touch_move(&mut self, id: u32, x: u32, y: u32, pressure: u32) -> Result<()> {
        self.command(format_args!("m {id} {x} {y} {pressure}\n"))
    }

    /// Touch up: lift contact `id`. (`u <id>`)
    pub fn touch_up(&mut self, id: u32) -> Result<()> {
        self.command(format_args!("u {id}\n"))
    }

    /// Commit the queued touch state. (`c`)
    pub fn commit(&mut self) -> Result<()> {
        self.command(format_args!("c\n"))
    }

    /// Wait `millis` milliseconds. (`w <ms>`)
    pub fn wait(&mut self, millis: u32) -> Result<()> {
        self.command(format_args!("w {millis}\n"))
    }

    /// Reset all contacts. (`r`)
    pub fn reset(&mut self) -> Result<()> {
        self.command(format_args!("r\n"))
    }

    /// Convenience: a single tap (down, commit, up, commit) of contact `0`.
    pub fn tap(&mut self, x: u32, y: u32, pressure: u32) -> Result<()> {
        self.touch_down(0, x, y, pressure)?;
        self.commit()?;
        self.touch_up(0)?;
        self.commit()
    }

    fn command(&mut self, args: std::fmt::Arguments) -> Result<()> {
        self.writer.write_fmt(args).map_err(RustADBError::IOError)?;
        self.writer.flush().map_err(RustADBError::IOError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_banner() {
        let banner = MinitouchBanner::parse("v 1\n^ 10 4095 4095 255\n$ 12345\n").unwrap();
        assert_eq!(banner.version, 1);
        assert_eq!(banner.max_contacts, 10);
        assert_eq!(banner.max_x, 4095);
        assert_eq!(banner.max_y, 4095);
        assert_eq!(banner.max_pressure, 255);
        assert_eq!(banner.pid, 12345);
    }

    #[test]
    fn rejects_incomplete_banner() {
        assert!(MinitouchBanner::parse("v 1\n").is_err());
    }

    #[test]
    fn serializes_commands() {
        let mut buf = Vec::new();
        {
            let mut mt = Minitouch::new(&mut buf);
            mt.touch_down(0, 100, 200, 50).unwrap();
            mt.commit().unwrap();
            mt.touch_move(0, 110, 210, 50).unwrap();
            mt.touch_up(0).unwrap();
            mt.commit().unwrap();
        }
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "d 0 100 200 50\nc\nm 0 110 210 50\nu 0\nc\n"
        );
    }

    #[test]
    fn tap_sequence() {
        let mut buf = Vec::new();
        {
            let mut mt = Minitouch::new(&mut buf);
            mt.tap(5, 6, 100).unwrap();
        }
        assert_eq!(String::from_utf8(buf).unwrap(), "d 0 5 6 100\nc\nu 0\nc\n");
    }
}
