use std::fmt;

/// Result type used throughout this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Errors produced while reading or writing save files.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A section magic value did not match the expected value.
    InvalidMagic {
        /// Name of the section being read.
        section: &'static str,
        /// The magic value found in the data.
        found: u32,
    },
    /// The data is structurally invalid.
    InvalidData(String),
    /// Attempted to read past the end of the data.
    UnexpectedEof,
    /// An invalid argument was passed (e.g. reading more than 32 bits at once).
    InvalidArgument(String),
    /// A Huffman encoded item code could not be decoded.
    InvalidHuffmanCode {
        /// Index of the character (0-3) that failed to decode.
        index: usize,
    },
    /// A character could not be Huffman encoded.
    UnencodableCharacter(u8),
    /// No game data is available for the requested save version.
    UnsupportedVersion {
        /// The requested version.
        version: u32,
        /// Versions that are available.
        available: Vec<u32>,
    },
    /// An item code is not present in the game data.
    UnknownItem(crate::ItemCode),
    /// A stat ID is not present in the game data.
    UnknownStat(crate::StatId),
    /// A string does not fit into its fixed size field.
    StringTooLong {
        /// Maximum number of bytes available.
        max: usize,
        /// Number of bytes required.
        actual: usize,
    },
    /// Error while parsing an item, with context about the item.
    Item {
        /// Item code (may be zero if the failure happened before the code was read).
        code: crate::ItemCode,
        /// Item flags.
        flags: u32,
        /// Whether the item uses the compact format.
        compact: bool,
        /// Item quality.
        quality: u8,
        /// Bit position where the error was detected.
        bit_position: usize,
        /// Underlying error.
        source: Box<Error>,
    },
    /// I/O error while loading external data files.
    Io(std::io::Error),
}

impl Error {
    pub(crate) fn invalid_data(msg: impl Into<String>) -> Self {
        Error::InvalidData(msg.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidMagic { section, found } => {
                write!(f, "invalid {section} magic: 0x{found:X}")
            }
            Error::InvalidData(msg) => f.write_str(msg),
            Error::UnexpectedEof => f.write_str("attempted to read past end of data"),
            Error::InvalidArgument(msg) => f.write_str(msg),
            Error::InvalidHuffmanCode { index } => {
                write!(f, "invalid Huffman code encountered at character {index}")
            }
            Error::UnencodableCharacter(c) => {
                write!(f, "character 0x{c:02X} not in Huffman table")
            }
            Error::UnsupportedVersion { version, available } => {
                write!(f, "no txt data available for version {version}, available versions: ")?;
                for (i, v) in available.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{v}")?;
                }
                Ok(())
            }
            Error::UnknownItem(code) => write!(f, "unknown item code '{code}'"),
            Error::UnknownStat(id) => write!(f, "stat ID {} out of range", id.0),
            Error::StringTooLong { max, actual } => {
                write!(f, "string of {actual} bytes does not fit into {max} bytes")
            }
            Error::Item { code, flags, compact, quality, bit_position, source } => write!(
                f,
                "error parsing item code={code} flags=0x{flags:08X} compact={compact} quality={quality} at bit {bit_position}: {source}"
            ),
            Error::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Item { source, .. } => Some(source.as_ref()),
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
