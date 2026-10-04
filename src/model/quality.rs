use crate::bits::{BitReader, BitWriter};
use crate::error::Result;

/// Quality-specific item data. Which variant is present depends on the item quality
/// (and, for normal quality items, on the item type).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum QualityData {
    /// Inferior (low quality) item.
    Inferior {
        /// Low quality type (0-7).
        file_index: u8,
    },
    /// Superior item.
    Superior {
        /// Superior type (0-7).
        file_index: u8,
    },
    /// Magic item.
    Magic {
        /// Magic prefix ID (0 = none).
        prefix: u16,
        /// Magic suffix ID (0 = none).
        suffix: u16,
    },
    /// Set or unique item.
    SetUnique {
        /// Set or unique item row (0-4095).
        file_index: u16,
    },
    /// Rare or crafted item.
    RareCraft(RareCraftData),
    /// Tempered item.
    Tempered {
        /// Rare name prefix ID.
        name_prefix: u8,
        /// Rare name suffix ID.
        name_suffix: u8,
    },
    /// Normal quality charm (has a magic prefix or a suffix).
    NormalCharm {
        /// Whether the affix is a prefix.
        has_prefix: bool,
        /// Prefix ID (when `has_prefix`).
        prefix: u16,
        /// Suffix ID (when not `has_prefix`).
        suffix: u16,
    },
    /// Normal quality monster body part.
    NormalBodyPart {
        /// Monster ID (0-1023).
        file_index: u16,
    },
    /// Normal quality scroll or book.
    NormalScrollBook {
        /// Spell type (0-31).
        spell_type: u8,
    },
}

/// Rare/crafted item affixes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RareCraftData {
    /// Rare name prefix ID.
    pub name_prefix: u8,
    /// Rare name suffix ID.
    pub name_suffix: u8,
    /// Up to 3 magic prefix IDs (0 = unused).
    pub prefixes: [u16; 3],
    /// Up to 3 magic suffix IDs (0 = unused).
    pub suffixes: [u16; 3],
}

fn read_prefix(r: &mut BitReader<'_>, offset: i32) -> Result<u16> {
    let raw = r.read_bits(11)?;
    Ok(if raw != 0 { (raw as i32).wrapping_add(offset) as u16 } else { 0 })
}

fn write_prefix(w: &mut BitWriter<'_>, prefix: u16, offset: i32) -> Result<()> {
    let prefix = prefix as i32;
    let raw = if prefix > offset { prefix - offset } else { prefix };
    w.write_bits(raw.clamp(0, 2047) as u32, 11)
}

impl QualityData {
    /// The file index for qualities that have one (inferior, superior, set/unique, body part).
    pub fn file_index(&self) -> Option<u16> {
        match *self {
            QualityData::Inferior { file_index } | QualityData::Superior { file_index } => Some(file_index as u16),
            QualityData::SetUnique { file_index } | QualityData::NormalBodyPart { file_index } => Some(file_index),
            _ => None,
        }
    }

    pub(crate) fn read_inferior(r: &mut BitReader<'_>) -> Result<Self> {
        Ok(QualityData::Inferior { file_index: r.read_bits(3)? as u8 })
    }

    pub(crate) fn read_superior(r: &mut BitReader<'_>) -> Result<Self> {
        Ok(QualityData::Superior { file_index: r.read_bits(3)? as u8 })
    }

    pub(crate) fn read_magic(r: &mut BitReader<'_>, offset: i32) -> Result<Self> {
        Ok(QualityData::Magic { prefix: read_prefix(r, offset)?, suffix: r.read_bits(11)? as u16 })
    }

    pub(crate) fn read_set_unique(r: &mut BitReader<'_>) -> Result<Self> {
        Ok(QualityData::SetUnique { file_index: r.read_bits(12)? as u16 })
    }

    pub(crate) fn read_rare_craft(r: &mut BitReader<'_>, offset: i32) -> Result<Self> {
        let mut data = RareCraftData { name_prefix: r.read_u8()?, name_suffix: r.read_u8()?, ..Default::default() };
        for i in 0..3 {
            if r.read_bool()? {
                data.prefixes[i] = read_prefix(r, offset)?;
            }
            if r.read_bool()? {
                data.suffixes[i] = r.read_bits(11)? as u16;
            }
        }
        Ok(QualityData::RareCraft(data))
    }

    pub(crate) fn read_tempered(r: &mut BitReader<'_>) -> Result<Self> {
        Ok(QualityData::Tempered { name_prefix: r.read_u8()?, name_suffix: r.read_u8()? })
    }

    pub(crate) fn read_charm(r: &mut BitReader<'_>, offset: i32) -> Result<Self> {
        let has_prefix = r.read_bool()?;
        Ok(if has_prefix {
            QualityData::NormalCharm { has_prefix, prefix: read_prefix(r, offset)?, suffix: 0 }
        } else {
            QualityData::NormalCharm { has_prefix, prefix: 0, suffix: r.read_bits(11)? as u16 }
        })
    }

    pub(crate) fn read_body_part(r: &mut BitReader<'_>) -> Result<Self> {
        Ok(QualityData::NormalBodyPart { file_index: r.read_bits(10)? as u16 })
    }

    pub(crate) fn read_scroll_book(r: &mut BitReader<'_>) -> Result<Self> {
        Ok(QualityData::NormalScrollBook { spell_type: r.read_bits(5)? as u8 })
    }

    /// Writes the quality data. `magic_prefix_offset` is subtracted from prefix IDs.
    pub fn write(&self, w: &mut BitWriter<'_>, magic_prefix_offset: i32) -> Result<()> {
        match self {
            QualityData::Inferior { file_index } | QualityData::Superior { file_index } => {
                w.write_bits((*file_index).min(7) as u32, 3)
            }
            QualityData::Magic { prefix, suffix } => {
                write_prefix(w, *prefix, magic_prefix_offset)?;
                w.write_bits((*suffix).min(2047) as u32, 11)
            }
            QualityData::SetUnique { file_index } => w.write_bits((*file_index).min(4095) as u32, 12),
            QualityData::RareCraft(data) => {
                w.write_u8(data.name_prefix)?;
                w.write_u8(data.name_suffix)?;
                for i in 0..3 {
                    let prefix = data.prefixes[i];
                    w.write_bool(prefix != 0)?;
                    if prefix != 0 {
                        write_prefix(w, prefix, magic_prefix_offset)?;
                    }
                    let suffix = data.suffixes[i];
                    w.write_bool(suffix != 0)?;
                    if suffix != 0 {
                        w.write_bits(suffix.min(2047) as u32, 11)?;
                    }
                }
                Ok(())
            }
            QualityData::Tempered { name_prefix, name_suffix } => {
                w.write_u8(*name_prefix)?;
                w.write_u8(*name_suffix)
            }
            QualityData::NormalCharm { has_prefix, prefix, suffix } => {
                w.write_bool(*has_prefix)?;
                if *has_prefix {
                    write_prefix(w, *prefix, magic_prefix_offset)
                } else {
                    w.write_bits((*suffix).min(2047) as u32, 11)
                }
            }
            QualityData::NormalBodyPart { file_index } => w.write_bits((*file_index).min(1023) as u32, 10),
            QualityData::NormalScrollBook { spell_type } => w.write_bits((*spell_type).min(31) as u32, 5),
        }
    }
}
