//! String helpers mirroring the encoding behaviour of the game files.

use std::borrow::Cow;

/// Decodes a null-terminated UTF-8 field (invalid sequences become U+FFFD).
pub(crate) fn decode_fixed_utf8(bytes: &[u8]) -> String {
    decode_fixed_utf8_cow(bytes).into_owned()
}

/// Decodes a null-terminated UTF-8 field without allocating when it is valid.
pub(crate) fn decode_fixed_utf8_cow(bytes: &[u8]) -> Cow<'_, str> {
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..len])
}

/// Decodes a null-terminated ASCII field (bytes >= 0x80 become '?').
pub(crate) fn decode_fixed_ascii(bytes: &[u8]) -> String {
    bytes.iter().take_while(|&&b| b != 0).map(|&b| if b < 0x80 { b as char } else { '?' }).collect()
}

/// Truncates a string to at most `max_units` UTF-16 code units.
///
/// If the cut splits a surrogate pair the dangling half is replaced with U+FFFD,
/// which is what encoding a lone surrogate produces.
pub(crate) fn truncate_utf16(s: &str, max_units: usize) -> Cow<'_, str> {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        let n = c.len_utf16();
        if units + n > max_units {
            if units < max_units {
                let mut out = s[..i].to_owned();
                out.push('\u{FFFD}');
                return Cow::Owned(out);
            }
            return Cow::Borrowed(&s[..i]);
        }
        units += n;
    }
    Cow::Borrowed(s)
}

/// Writes `s` as UTF-8 into `dest` (which is zeroed first).
pub(crate) fn write_utf8_field(dest: &mut [u8], s: &str) -> crate::Result<()> {
    if s.len() > dest.len() {
        return Err(crate::Error::StringTooLong { max: dest.len(), actual: s.len() });
    }
    dest.fill(0);
    dest[..s.len()].copy_from_slice(s.as_bytes());
    Ok(())
}

/// Writes `s` as ASCII into `dest` (zeroed first); each non-ASCII UTF-16 unit becomes `?`.
pub(crate) fn write_ascii_field(dest: &mut [u8], s: &str) -> crate::Result<()> {
    let len = s.encode_utf16().count();
    if len > dest.len() {
        return Err(crate::Error::StringTooLong { max: dest.len(), actual: len });
    }
    dest.fill(0);
    for (d, unit) in dest.iter_mut().zip(s.encode_utf16()) {
        *d = if unit < 0x80 { unit as u8 } else { b'?' };
    }
    Ok(())
}
