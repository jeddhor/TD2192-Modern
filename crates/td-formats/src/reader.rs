use crate::{FormatError, Result};

/// Bounds-checked little-endian cursor over a byte slice.
///
/// Every read returns [`FormatError::Truncated`] instead of panicking when the data runs out.
#[derive(Debug, Clone)]
pub struct ByteReader<'a> {
    data: &'a [u8],
    pos: usize,
    format: &'static str,
}

impl<'a> ByteReader<'a> {
    /// Creates a reader over `data`; `format` names the format in error messages.
    pub fn new(format: &'static str, data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            format,
        }
    }

    /// Current offset from the start of the data.
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Bytes left to read.
    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    /// Moves to an absolute offset (which may equal the data length).
    pub fn seek(&mut self, pos: usize) -> Result<()> {
        if pos > self.data.len() {
            return Err(self.truncated(pos - self.data.len()));
        }
        self.pos = pos;
        Ok(())
    }

    /// Reads `n` bytes.
    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len());
        match end {
            Some(end) => {
                let out = &self.data[self.pos..end];
                self.pos = end;
                Ok(out)
            }
            None => Err(self.truncated(n - self.remaining())),
        }
    }

    /// Reads one byte.
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }

    /// Reads a little-endian `u16`.
    pub fn u16_le(&mut self) -> Result<u16> {
        let b = self.bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    /// Reads a little-endian 24-bit value (used by Creative Voice File block lengths).
    pub fn u24_le(&mut self) -> Result<u32> {
        let b = self.bytes(3)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], 0]))
    }

    /// Reads a little-endian `u32`.
    pub fn u32_le(&mut self) -> Result<u32> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Builds an [`FormatError::Invalid`] at the current offset.
    pub fn invalid(&self, detail: impl Into<String>) -> FormatError {
        FormatError::Invalid {
            format: self.format,
            offset: self.pos,
            detail: detail.into(),
        }
    }

    fn truncated(&self, needed: usize) -> FormatError {
        FormatError::Truncated {
            format: self.format,
            offset: self.pos,
            needed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_little_endian_values() {
        let data = [0x01, 0x34, 0x12, 0x56, 0x34, 0x12, 0x78, 0x56, 0x34, 0x12];
        let mut r = ByteReader::new("test", &data);
        assert_eq!(r.u8().unwrap(), 0x01);
        assert_eq!(r.u16_le().unwrap(), 0x1234);
        assert_eq!(r.u24_le().unwrap(), 0x12_3456);
        assert_eq!(r.u32_le().unwrap(), 0x1234_5678);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn reports_truncation_instead_of_panicking() {
        let mut r = ByteReader::new("test", &[1, 2, 3]);
        r.u16_le().unwrap();
        assert_eq!(
            r.u16_le(),
            Err(FormatError::Truncated {
                format: "test",
                offset: 2,
                needed: 1
            })
        );
        assert!(r.bytes(usize::MAX).is_err());
        assert!(r.seek(4).is_err());
        assert!(r.seek(3).is_ok());
    }
}
