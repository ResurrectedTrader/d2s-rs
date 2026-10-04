//! Read and write Diablo 2 save files: `.d2s` character saves and `.d2i` shared stash files.
//!
//! Supports the original Lord of Destruction format (version 96) and Diablo 2
//! Resurrected formats (versions 97-105), including conversion between them.
//! Reading and writing an unmodified file produces byte-identical output.
//!
//! ```no_run
//! use d2s::{D2Save, StatId};
//!
//! # fn main() -> d2s::Result<()> {
//! let bytes = std::fs::read("MyCharacter.d2s")?;
//! let mut save = D2Save::from_bytes(&bytes)?;
//! println!("{} (level {})", save.name(), save.character.level);
//!
//! save.stats.set(StatId::STRENGTH, 200);
//! for item in &save.items {
//!     println!("{} {:?}", item.code, item.quality);
//! }
//!
//! std::fs::write("MyCharacter.d2s", save.to_bytes()?)?;
//! # Ok(())
//! # }
//! ```
//!
//! Parsing items requires game data (bit widths of stats, item classifications).
//! The data for versions 96, 97, 99 and 105 is embedded (feature `embedded-data`,
//! enabled by default); modded games can load their own .txt files with
//! [`TxtData::from_dir`] and use the `*_with` methods.

#![warn(missing_docs)]

pub mod bits;
pub mod checksum;
pub mod data;
mod enums;
mod error;
pub mod huffman;
pub mod model;
pub mod overlay;
mod stat_id;
mod text;

pub use data::{ExternalData, ItemTypeInfo, StatInfo, TxtData};
pub use enums::*;
pub use error::{Error, Result};
pub use model::*;
pub use stat_id::StatId;
