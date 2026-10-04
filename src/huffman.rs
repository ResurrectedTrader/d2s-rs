//! Huffman coding of item codes used by D2R (save version > 96).

use crate::bits::{BitReader, BitWriter};
use crate::error::{Error, Result};

/// (symbol, code bits MSB first, code length)
const TABLE: [(u8, u32, u32); 37] = [
    (b'0', 0b11111011, 8),
    (b' ', 0b10, 2),
    (b'1', 0b1111100, 7),
    (b'2', 0b001100, 6),
    (b'3', 0b1101101, 7),
    (b'4', 0b11111010, 8),
    (b'5', 0b00010110, 8),
    (b'6', 0b1101111, 7),
    (b'7', 0b01111, 5),
    (b'8', 0b000100, 6),
    (b'9', 0b01110, 5),
    (b'a', 0b11110, 5),
    (b'b', 0b0101, 4),
    (b'c', 0b01000, 5),
    (b'd', 0b110001, 6),
    (b'e', 0b110000, 6),
    (b'f', 0b010011, 6),
    (b'g', 0b11010, 5),
    (b'h', 0b00011, 5),
    (b'i', 0b1111110, 7),
    (b'j', 0b000101110, 9),
    (b'k', 0b010010, 6),
    (b'l', 0b11101, 5),
    (b'm', 0b01101, 5),
    (b'n', 0b001101, 6),
    (b'o', 0b1111111, 7),
    (b'p', 0b11001, 5),
    (b'q', 0b11011001, 8),
    (b'r', 0b11100, 5),
    (b's', 0b0010, 4),
    (b't', 0b01100, 5),
    (b'u', 0b00001, 5),
    (b'v', 0b1101110, 7),
    (b'w', 0b00000, 5),
    (b'x', 0b00111, 5),
    (b'y', 0b0001010, 7),
    (b'z', 0b11011000, 8),
];

const MAX_CODE_LENGTH: u32 = 9;

const fn reverse_bits(value: u32, length: u32) -> u32 {
    let mut result = 0;
    let mut v = value;
    let mut i = 0;
    while i < length {
        result = (result << 1) | (v & 1);
        v >>= 1;
        i += 1;
    }
    result
}

/// Encode lookup indexed by byte: (bits in stream order, length). Length 0 = not encodable.
const ENCODE: [(u32, u32); 256] = {
    let mut table = [(0u32, 0u32); 256];
    let mut i = 0;
    while i < TABLE.len() {
        let (sym, bits, len) = TABLE[i];
        table[sym as usize] = (reverse_bits(bits, len), len);
        i += 1;
    }
    table
};

/// Decode lookup indexed by the next 9 bits of the stream (first bit read = bit 0):
/// (symbol, length). Length 0 = invalid code.
const DECODE: [(u8, u8); 512] = {
    let mut table = [(0u8, 0u8); 512];
    let mut i = 0;
    while i < TABLE.len() {
        let (sym, bits, len) = TABLE[i];
        let stream = reverse_bits(bits, len);
        let mut fill = 0u32;
        while fill < (1 << (MAX_CODE_LENGTH - len)) {
            table[(stream | (fill << len)) as usize] = (sym, len as u8);
            fill += 1;
        }
        i += 1;
    }
    table
};

/// Huffman encodes an item code (4 ASCII characters, little-endian), stopping at a null character.
pub fn encode(writer: &mut BitWriter<'_>, code: u32) -> Result<()> {
    for ch in code.to_le_bytes() {
        if ch == 0 {
            break;
        }
        let (bits, len) = ENCODE[ch as usize];
        if len == 0 {
            return Err(Error::UnencodableCharacter(ch));
        }
        writer.write_bits(bits, len)?;
    }
    Ok(())
}

/// Decodes a Huffman encoded item code (always 4 characters).
pub fn decode(reader: &mut BitReader<'_>) -> Result<u32> {
    let mut result = 0u32;
    for index in 0..4 {
        let available = reader.bits_remaining().min(MAX_CODE_LENGTH as usize) as u32;
        let peek = reader.peek_bits(available)?;
        let (sym, len) = DECODE[peek as usize];
        if len == 0 || len as u32 > available {
            return Err(if available < MAX_CODE_LENGTH {
                Error::UnexpectedEof
            } else {
                Error::InvalidHuffmanCode { index }
            });
        }
        reader.skip_bits(len as usize);
        result |= (sym as u32) << (index * 8);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_all_symbols() {
        for chunk in TABLE.chunks(4) {
            let mut bytes = [b' '; 4];
            for (i, (s, _, _)) in chunk.iter().enumerate() {
                bytes[i] = *s;
            }
            let code = u32::from_le_bytes(bytes);
            let mut buf = Vec::new();
            encode(&mut BitWriter::new(&mut buf), code).unwrap();
            assert_eq!(decode(&mut BitReader::new(&buf)).unwrap(), code);
        }
    }
}
