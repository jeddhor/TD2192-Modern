//! Minimal MZ (DOS executable) header reader and Pascal ShortString helpers, used to extract
//! text and tables embedded in the original executables at runtime (ADR-0003, MDG §9.1.4).
//!
//! Address notation (ADG §7.8): an image-relative `seg:off` maps to the file offset
//! `header_bytes + seg * 16 + off`.

use crate::{ByteReader, FormatError, Result};

/// The fields of an MZ header that the port needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MzHeader {
    /// Size of the header in bytes (`paragraphs * 16`).
    pub header_bytes: usize,
    /// Size of the load image + header as stated by the header, in bytes.
    pub image_end: usize,
    pub relocations: u16,
    pub min_alloc_paragraphs: u16,
    pub max_alloc_paragraphs: u16,
    pub initial_ss: u16,
    pub initial_sp: u16,
    pub initial_ip: u16,
    pub initial_cs: u16,
}

impl MzHeader {
    /// Parses the header at the start of an executable.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = ByteReader::new("mz", data);
        if r.bytes(2)? != b"MZ" {
            return Err(FormatError::BadMagic { format: "mz" });
        }
        let last_page = r.u16_le()? as usize;
        let pages = r.u16_le()? as usize;
        let relocations = r.u16_le()?;
        let header_paragraphs = r.u16_le()? as usize;
        let min_alloc_paragraphs = r.u16_le()?;
        let max_alloc_paragraphs = r.u16_le()?;
        let initial_ss = r.u16_le()?;
        let initial_sp = r.u16_le()?;
        let _checksum = r.u16_le()?;
        let initial_ip = r.u16_le()?;
        let initial_cs = r.u16_le()?;
        let image_end = match (pages, last_page) {
            (0, _) => 0,
            (p, 0) => p * 512,
            (p, l) => (p - 1) * 512 + l,
        };
        Ok(Self {
            header_bytes: header_paragraphs * 16,
            image_end,
            relocations,
            min_alloc_paragraphs,
            max_alloc_paragraphs,
            initial_ss,
            initial_sp,
            initial_ip,
            initial_cs,
        })
    }

    /// File offset of an image-relative `seg:off` address.
    pub fn file_offset(&self, seg: u16, off: u16) -> usize {
        self.header_bytes + seg as usize * 16 + off as usize
    }
}

/// Reads the Pascal ShortString (length byte + text) at `offset`.
pub fn pstring(data: &[u8], offset: usize) -> Result<&[u8]> {
    let mut r = ByteReader::new("pstring", data);
    r.seek(offset)?;
    let len = r.u8()? as usize;
    r.bytes(len)
}

/// Reads `count` consecutive ShortStrings starting at `offset`.
pub fn pstring_seq(data: &[u8], offset: usize, count: usize) -> Result<Vec<&[u8]>> {
    // Each string takes at least its length byte, so the data bounds the count (BUG-16).
    let mut out = Vec::with_capacity(count.min(data.len().saturating_sub(offset)));
    let mut at = offset;
    for _ in 0..count {
        let s = pstring(data, at)?;
        at += 1 + s.len();
        out.push(s);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_header_and_maps_addresses() {
        let mut h = vec![0u8; 64];
        h[..2].copy_from_slice(b"MZ");
        h[2..4].copy_from_slice(&0x1F0u16.to_le_bytes()); // last page bytes
        h[4..6].copy_from_slice(&0x42u16.to_le_bytes()); // pages
        h[8..10].copy_from_slice(&0x22u16.to_le_bytes()); // header paragraphs
        h[20..22].copy_from_slice(&0x1FEu16.to_le_bytes()); // IP
        let m = MzHeader::parse(&h).unwrap();
        assert_eq!(m.header_bytes, 544);
        assert_eq!(
            m.image_end, 33_776,
            "td1.exe's header values give its file size"
        );
        assert_eq!(m.initial_ip, 0x1FE);
        assert_eq!(m.file_offset(1, 2), 544 + 18);
        assert!(MzHeader::parse(b"ZM").is_err());
    }

    #[test]
    fn reads_short_strings() {
        let data = b"\x03abc\x02de\x00";
        assert_eq!(pstring(data, 0).unwrap(), b"abc");
        assert_eq!(pstring_seq(data, 0, 3).unwrap(), [&b"abc"[..], b"de", b""]);
        assert!(pstring(data, 8).is_err());
        assert!(pstring(b"\x05ab", 0).is_err());
    }

    #[test]
    fn huge_pstring_counts_fail_without_reserving() {
        // BUG-16 (M12-06 fuzzing): the count used to size the vector up front.
        assert!(pstring_seq(&[], 0, usize::MAX).is_err());
        assert!(pstring_seq(&[0, 0], 0, 1 << 59).is_err());
        assert_eq!(pstring_seq(&[0, 0], 0, 2).unwrap().len(), 2);
    }
}
