//! Low-level bit reader and writer.
//!
//! Bits are read and written LSB-first within each byte (little-endian bit order),
//! which is the order used by every bit-packed structure in Diablo 2 save files.

use crate::error::{Error, Result};
use crate::text;

/// Reads bits from a byte slice.
#[derive(Clone, Debug)]
pub struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BitReader<'a> {
    /// Creates a reader positioned at the start of `data`.
    pub fn new(data: &'a [u8]) -> Self {
        BitReader { data, pos: 0 }
    }

    /// Current bit position.
    #[inline]
    pub fn bit_position(&self) -> usize {
        self.pos
    }

    /// Current byte position (bit position / 8).
    #[inline]
    pub fn byte_position(&self) -> usize {
        self.pos >> 3
    }

    /// Number of bits remaining (zero if the position is past the end).
    #[inline]
    pub fn bits_remaining(&self) -> usize {
        (self.data.len() << 3).saturating_sub(self.pos)
    }

    /// Sets the bit position.
    #[inline]
    pub fn set_bit_position(&mut self, pos: usize) {
        self.pos = pos;
    }

    /// Sets the byte position.
    #[inline]
    pub fn set_byte_position(&mut self, pos: usize) {
        self.pos = pos << 3;
    }

    /// Reads up to 32 bits as an unsigned value.
    #[inline]
    pub fn read_bits(&mut self, count: u32) -> Result<u32> {
        if count == 0 {
            return Ok(0);
        }
        if count > 32 {
            return Err(Error::InvalidArgument("cannot read more than 32 bits at once".into()));
        }
        let end = self.pos + count as usize;
        if end > self.data.len() << 3 {
            return Err(Error::UnexpectedEof);
        }
        let byte = self.pos >> 3;
        let offset = (self.pos & 7) as u32;
        let word = match self.data.get(byte..byte + 8) {
            Some(chunk) => u64::from_le_bytes(chunk.try_into().unwrap()),
            None => {
                let mut tmp = [0u8; 8];
                let tail = &self.data[byte..];
                tmp[..tail.len()].copy_from_slice(tail);
                u64::from_le_bytes(tmp)
            }
        };
        self.pos = end;
        Ok(((word >> offset) & ((1u64 << count) - 1)) as u32)
    }

    /// Reads up to 32 bits as a sign-extended value.
    pub fn read_signed_bits(&mut self, count: u32) -> Result<i32> {
        if count == 0 {
            return Ok(0);
        }
        let raw = self.read_bits(count)?;
        if count < 32 && raw & (1 << (count - 1)) != 0 {
            Ok((raw | !((1u32 << count) - 1)) as i32)
        } else {
            Ok(raw as i32)
        }
    }

    /// Peeks at up to 32 bits without advancing.
    pub fn peek_bits(&self, count: u32) -> Result<u32> {
        self.clone().read_bits(count)
    }

    /// Reads a single bit.
    #[inline]
    pub fn read_bool(&mut self) -> Result<bool> {
        Ok(self.read_bits(1)? != 0)
    }

    /// Reads 8 bits.
    #[inline]
    pub fn read_u8(&mut self) -> Result<u8> {
        Ok(self.read_bits(8)? as u8)
    }

    /// Reads 16 bits.
    #[inline]
    pub fn read_u16(&mut self) -> Result<u16> {
        Ok(self.read_bits(16)? as u16)
    }

    /// Reads 32 bits.
    #[inline]
    pub fn read_u32(&mut self) -> Result<u32> {
        self.read_bits(32)
    }

    /// Reads 64 bits.
    #[inline]
    pub fn read_u64(&mut self) -> Result<u64> {
        let low = self.read_bits(32)? as u64;
        let high = self.read_bits(32)? as u64;
        Ok(high << 32 | low)
    }

    fn require_aligned(&self, what: &str) -> Result<()> {
        if self.pos & 7 != 0 {
            return Err(Error::InvalidArgument(format!("{what} requires byte alignment")));
        }
        Ok(())
    }

    /// Reads `count` bytes as a borrowed slice. Must be byte-aligned.
    pub fn read_slice(&mut self, count: usize) -> Result<&'a [u8]> {
        self.require_aligned("read_bytes")?;
        let start = self.pos >> 3;
        let slice = self.data.get(start..start + count).ok_or(Error::UnexpectedEof)?;
        self.pos += count << 3;
        Ok(slice)
    }

    /// Fills `dest` with bytes. Must be byte-aligned.
    pub fn read_bytes(&mut self, dest: &mut [u8]) -> Result<()> {
        dest.copy_from_slice(self.read_slice(dest.len())?);
        Ok(())
    }

    /// Reads a fixed size byte array. Must be byte-aligned.
    pub fn read_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut out = [0u8; N];
        self.read_bytes(&mut out)?;
        Ok(out)
    }

    /// Aligns to the next byte boundary.
    #[inline]
    pub fn align_to_byte(&mut self) {
        self.pos = (self.pos + 7) & !7;
    }

    /// Skips `count` bits.
    pub fn skip_bits(&mut self, count: usize) {
        self.pos += count;
    }

    /// Reads a null-terminated string of 8-bit (UTF-8) characters, at most 64 bytes.
    pub fn read_8bit_string(&mut self) -> Result<String> {
        let mut buf = [0u8; 64];
        let mut len = 0;
        while len < buf.len() {
            let ch = self.read_bits(8)? as u8;
            if ch == 0 {
                break;
            }
            buf[len] = ch;
            len += 1;
        }
        Ok(String::from_utf8_lossy(&buf[..len]).into_owned())
    }

    /// Reads a null-terminated string of 7-bit characters, at most 32 characters.
    pub fn read_7bit_string(&mut self) -> Result<String> {
        let mut s = String::new();
        while s.len() < 32 {
            let ch = self.read_bits(7)? as u8;
            if ch == 0 {
                break;
            }
            s.push(ch as char);
        }
        Ok(s)
    }

    /// Reads a fixed-length UTF-8 string, stopping at the first null byte.
    pub fn read_fixed_string(&mut self, len: usize) -> Result<String> {
        Ok(text::decode_fixed_utf8(self.read_slice(len)?))
    }

    /// Returns the bytes from the current (rounded up) byte position to the end.
    pub fn remaining_bytes(&self) -> &'a [u8] {
        let start = ((self.pos + 7) >> 3).min(self.data.len());
        &self.data[start..]
    }
}

/// Writes bits into a growable byte buffer.
///
/// Bytes are appended to the supplied vector starting at its length when the
/// writer was created; positions reported by the writer are relative to that point.
#[derive(Debug)]
pub struct BitWriter<'a> {
    buf: &'a mut Vec<u8>,
    start: usize,
    pos: usize,
}

impl<'a> BitWriter<'a> {
    /// Creates a writer appending to `buf`.
    pub fn new(buf: &'a mut Vec<u8>) -> Self {
        let start = buf.len();
        BitWriter { buf, start, pos: 0 }
    }

    /// Current bit position.
    #[inline]
    pub fn bit_position(&self) -> usize {
        self.pos
    }

    /// Current byte position (bit position / 8).
    #[inline]
    pub fn byte_position(&self) -> usize {
        self.pos >> 3
    }

    /// Number of bytes written (rounded up).
    #[inline]
    pub fn bytes_written(&self) -> usize {
        (self.pos + 7) >> 3
    }

    /// The bytes written so far.
    pub fn written(&self) -> &[u8] {
        &self.buf[self.start..]
    }

    /// Mutable access to the bytes written so far.
    pub fn written_mut(&mut self) -> &mut [u8] {
        &mut self.buf[self.start..]
    }

    #[inline]
    fn grow_to(&mut self, bit_end: usize) {
        let needed = self.start + ((bit_end + 7) >> 3);
        if self.buf.len() < needed {
            self.buf.resize(needed, 0);
        }
    }

    /// Writes the low `count` bits (up to 32) of `value`.
    #[inline]
    pub fn write_bits(&mut self, value: u32, count: u32) -> Result<()> {
        if count == 0 {
            return Ok(());
        }
        if count > 32 {
            return Err(Error::InvalidArgument("cannot write more than 32 bits at once".into()));
        }
        let end = self.pos + count as usize;
        self.grow_to(end);
        let value = (value as u64) & ((1u64 << count) - 1);
        let offset = self.pos & 7;
        let byte = self.start + (self.pos >> 3);
        let shifted = value << offset;
        let nbytes = (offset + count as usize + 7) >> 3;
        for (i, b) in self.buf[byte..byte + nbytes].iter_mut().enumerate() {
            *b |= (shifted >> (i * 8)) as u8;
        }
        self.pos = end;
        Ok(())
    }

    /// Writes the low `count` bits (up to 32) of a signed value.
    #[inline]
    pub fn write_signed_bits(&mut self, value: i32, count: u32) -> Result<()> {
        self.write_bits(value as u32, count)
    }

    /// Writes a single bit.
    #[inline]
    pub fn write_bool(&mut self, value: bool) -> Result<()> {
        self.write_bits(value as u32, 1)
    }

    /// Writes 8 bits.
    #[inline]
    pub fn write_u8(&mut self, value: u8) -> Result<()> {
        self.write_bits(value as u32, 8)
    }

    /// Writes 16 bits.
    #[inline]
    pub fn write_u16(&mut self, value: u16) -> Result<()> {
        self.write_bits(value as u32, 16)
    }

    /// Writes 32 bits.
    #[inline]
    pub fn write_u32(&mut self, value: u32) -> Result<()> {
        self.write_bits(value, 32)
    }

    /// Writes 64 bits.
    #[inline]
    pub fn write_u64(&mut self, value: u64) -> Result<()> {
        self.write_bits(value as u32, 32)?;
        self.write_bits((value >> 32) as u32, 32)
    }

    fn require_aligned(&self, what: &str) -> Result<()> {
        if self.pos & 7 != 0 {
            return Err(Error::InvalidArgument(format!("{what} requires byte alignment")));
        }
        Ok(())
    }

    /// Writes raw bytes. Must be byte-aligned.
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        self.require_aligned("write_bytes")?;
        let at = self.start + (self.pos >> 3);
        self.pos += bytes.len() << 3;
        self.grow_to(self.pos);
        self.buf[at..at + bytes.len()].copy_from_slice(bytes);
        Ok(())
    }

    /// Aligns to the next byte boundary (padding with zero bits).
    #[inline]
    pub fn align_to_byte(&mut self) {
        self.pos = (self.pos + 7) & !7;
        self.grow_to(self.pos);
    }

    /// Skips `count` bits, leaving them zero.
    pub fn skip_bits(&mut self, count: usize) {
        self.pos += count;
        self.grow_to(self.pos);
    }

    /// Writes a null-terminated UTF-8 string (at most 192 bytes plus terminator).
    pub fn write_8bit_string(&mut self, s: &str) -> Result<()> {
        if s.len() > 192 {
            return Err(Error::StringTooLong { max: 192, actual: s.len() });
        }
        for &b in s.as_bytes() {
            self.write_bits(b as u32, 8)?;
        }
        self.write_bits(0, 8)
    }

    /// Writes a null-terminated string of 7-bit characters.
    ///
    /// Each UTF-16 code unit is masked to its low 7 bits.
    pub fn write_7bit_string(&mut self, s: &str) -> Result<()> {
        for unit in s.encode_utf16() {
            self.write_bits(unit as u32 & 0x7F, 7)?;
        }
        self.write_bits(0, 7)
    }

    /// Writes a fixed-length UTF-8 string padded with null bytes. Must be byte-aligned.
    pub fn write_fixed_string(&mut self, s: &str, len: usize) -> Result<()> {
        self.require_aligned("write_fixed_string")?;
        if s.len() > len {
            return Err(Error::StringTooLong { max: len, actual: s.len() });
        }
        let at = self.start + (self.pos >> 3);
        self.pos += len << 3;
        self.grow_to(self.pos);
        let dest = &mut self.buf[at..at + len];
        dest.fill(0);
        dest[..s.len()].copy_from_slice(s.as_bytes());
        Ok(())
    }

    /// Overwrites a little-endian `u16` at a byte offset relative to the writer start.
    pub fn patch_u16(&mut self, byte_offset: usize, value: u16) {
        let at = self.start + byte_offset;
        self.buf[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }

    /// Overwrites a little-endian `u32` at a byte offset relative to the writer start.
    pub fn patch_u32(&mut self, byte_offset: usize, value: u32) {
        let at = self.start + byte_offset;
        self.buf[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_round_trip() {
        let mut buf = Vec::new();
        let mut w = BitWriter::new(&mut buf);
        w.write_bits(5, 3).unwrap();
        w.write_bits(0x1FF, 9).unwrap();
        w.write_bits(0xDEADBEEF, 32).unwrap();
        w.write_signed_bits(-3, 5).unwrap();
        w.write_bool(true).unwrap();
        assert_eq!(w.bytes_written(), 7);
        let mut r = BitReader::new(&buf);
        assert_eq!(r.read_bits(3).unwrap(), 5);
        assert_eq!(r.read_bits(9).unwrap(), 0x1FF);
        assert_eq!(r.read_bits(32).unwrap(), 0xDEADBEEF);
        assert_eq!(r.read_signed_bits(5).unwrap(), -3);
        assert!(r.read_bool().unwrap());
        assert!(r.read_bits(8).is_err());
    }

    #[test]
    fn writer_appends() {
        let mut buf = vec![1, 2];
        let mut w = BitWriter::new(&mut buf);
        w.write_u16(0x0403).unwrap();
        w.align_to_byte();
        assert_eq!(w.written(), &[3, 4]);
        assert_eq!(buf, [1, 2, 3, 4]);
    }
}
