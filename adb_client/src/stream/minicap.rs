use std::io::{ErrorKind, Read};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::{Result, RustADBError};

/// Upper bound on a single minicap frame, guarding against a corrupt or hostile length prefix
/// (which would otherwise drive a multi-GiB allocation). Comfortably above any real frame.
const MAX_FRAME_LEN: usize = 64 * 1024 * 1024;

/// The global header (banner) sent once at the start of a `minicap` stream.
///
/// See the minicap wire format:
/// <https://github.com/DeviceFarmer/minicap#usage>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinicapHeader {
    /// Protocol version (currently `1`).
    pub version: u8,
    /// Size of this banner in bytes.
    pub header_size: u8,
    /// PID of the minicap process on the device.
    pub pid: u32,
    /// Real (physical) display width in pixels.
    pub real_width: u32,
    /// Real (physical) display height in pixels.
    pub real_height: u32,
    /// Virtual (projected) frame width in pixels.
    pub virtual_width: u32,
    /// Virtual (projected) frame height in pixels.
    pub virtual_height: u32,
    /// Display orientation in 90° steps (`0`, `1`, `2`, `3`).
    pub orientation: u8,
    /// Quirk bit flags (e.g. `DUMB`, `ALWAYS_UPRIGHT`, `TEAR`).
    pub quirk_flags: u8,
}

impl MinicapHeader {
    /// Number of meaningful bytes in the banner.
    pub const SIZE: usize = 24;

    /// Parse a banner from its raw bytes (at least [`MinicapHeader::SIZE`] bytes).
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < Self::SIZE {
            return Err(RustADBError::ConversionError);
        }

        let mut cursor = bytes;
        let version = cursor.read_u8()?;
        let header_size = cursor.read_u8()?;
        let pid = cursor.read_u32::<LittleEndian>()?;
        let real_width = cursor.read_u32::<LittleEndian>()?;
        let real_height = cursor.read_u32::<LittleEndian>()?;
        let virtual_width = cursor.read_u32::<LittleEndian>()?;
        let virtual_height = cursor.read_u32::<LittleEndian>()?;
        let orientation = cursor.read_u8()?;
        let quirk_flags = cursor.read_u8()?;

        Ok(Self {
            version,
            header_size,
            pid,
            real_width,
            real_height,
            virtual_width,
            virtual_height,
            orientation,
            quirk_flags,
        })
    }
}

/// Reader for a `minicap` stream: consumes the banner on construction, then yields
/// JPEG-encoded frames.
///
/// Wrap the stream returned by [`crate::server_device::ADBServerDevice::open_local`] with
/// `localabstract:minicap`. Frames are length-prefixed JPEGs; the geometry is in
/// [`MinicapStream::header`].
#[derive(Debug)]
pub struct MinicapStream<R: Read> {
    reader: R,
    header: MinicapHeader,
}

impl<R: Read> MinicapStream<R> {
    /// Read the global banner from `reader` and return the stream wrapper.
    pub fn new(mut reader: R) -> Result<Self> {
        // First two bytes are version + banner size; the banner may be larger than the fields
        // we understand, so read exactly `header_size` bytes and parse the known prefix.
        let mut prefix = [0u8; 2];
        reader.read_exact(&mut prefix)?;
        let header_size = prefix[1] as usize;
        if header_size < MinicapHeader::SIZE {
            return Err(RustADBError::ConversionError);
        }

        let mut banner = vec![0u8; header_size];
        banner[..2].copy_from_slice(&prefix);
        reader.read_exact(&mut banner[2..])?;

        let header = MinicapHeader::parse(&banner)?;
        Ok(Self { reader, header })
    }

    /// The banner describing the stream geometry.
    pub fn header(&self) -> &MinicapHeader {
        &self.header
    }

    /// Read the next JPEG frame (blocking). Each frame is a complete JPEG image.
    ///
    /// Returns `Ok(None)` when the stream ends cleanly (the device-side minicap closed the
    /// socket between frames), so callers can distinguish a graceful end from an I/O error.
    pub fn next_frame(&mut self) -> Result<Option<Vec<u8>>> {
        let length = match self.reader.read_u32::<LittleEndian>() {
            Ok(length) => length as usize,
            // No more frames: minicap closed the socket at a frame boundary.
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(RustADBError::IOError(e)),
        };

        if length > MAX_FRAME_LEN {
            return Err(RustADBError::ADBRequestFailed(format!(
                "minicap frame length {length} exceeds maximum {MAX_FRAME_LEN}"
            )));
        }

        let mut frame = vec![0u8; length];
        self.reader.read_exact(&mut frame)?;
        Ok(Some(frame))
    }

    /// Consume the stream and return the inner reader.
    pub fn into_inner(self) -> R {
        self.reader
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn sample_banner() -> Vec<u8> {
        let mut banner = Vec::new();
        banner.push(1); // version
        banner.push(MinicapHeader::SIZE as u8); // header size
        banner.extend_from_slice(&1234u32.to_le_bytes()); // pid
        banner.extend_from_slice(&1080u32.to_le_bytes()); // real width
        banner.extend_from_slice(&2340u32.to_le_bytes()); // real height
        banner.extend_from_slice(&720u32.to_le_bytes()); // virtual width
        banner.extend_from_slice(&1560u32.to_le_bytes()); // virtual height
        banner.push(1); // orientation
        banner.push(2); // quirk flags
        banner
    }

    #[test]
    fn parses_banner() {
        let header = MinicapHeader::parse(&sample_banner()).unwrap();
        assert_eq!(header.version, 1);
        assert_eq!(header.pid, 1234);
        assert_eq!(header.real_width, 1080);
        assert_eq!(header.real_height, 2340);
        assert_eq!(header.virtual_width, 720);
        assert_eq!(header.virtual_height, 1560);
        assert_eq!(header.orientation, 1);
        assert_eq!(header.quirk_flags, 2);
    }

    #[test]
    fn reads_header_and_frames() {
        let mut data = sample_banner();
        // Two frames: lengths followed by payloads.
        data.extend_from_slice(&3u32.to_le_bytes());
        data.extend_from_slice(&[0xFF, 0xD8, 0xFF]);
        data.extend_from_slice(&2u32.to_le_bytes());
        data.extend_from_slice(&[0xAA, 0xBB]);

        let mut stream = MinicapStream::new(Cursor::new(data)).unwrap();
        assert_eq!(stream.header().real_width, 1080);
        assert_eq!(stream.next_frame().unwrap(), Some(vec![0xFF, 0xD8, 0xFF]));
        assert_eq!(stream.next_frame().unwrap(), Some(vec![0xAA, 0xBB]));
        // Clean end of stream after the last frame.
        assert_eq!(stream.next_frame().unwrap(), None);
    }

    #[test]
    fn rejects_oversized_frame_length() {
        let mut data = sample_banner();
        data.extend_from_slice(&u32::MAX.to_le_bytes()); // absurd frame length
        let mut stream = MinicapStream::new(Cursor::new(data)).unwrap();
        assert!(stream.next_frame().is_err());
    }

    #[test]
    fn rejects_truncated_banner() {
        assert!(MinicapHeader::parse(&[1, 24, 0, 0]).is_err());
    }
}
