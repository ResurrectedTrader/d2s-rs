use std::ops::{Deref, DerefMut};

use crate::bits::{BitReader, BitWriter};
use crate::data::ExternalData;
use crate::enums::CharacterClass;
use crate::error::{Error, Result};
use crate::StatId;

/// A single stat (used for player, item and demon stats).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stat {
    /// Stat ID (row of ItemStatCost.txt).
    pub id: StatId,
    /// Layer/parameter (skill ID etc.).
    pub layer: i32,
    /// Stat value.
    pub value: i64,
}

impl Stat {
    /// Creates a stat with layer 0.
    pub const fn new(id: StatId, value: i64) -> Stat {
        Stat { id, layer: 0, value }
    }

    /// Creates a stat with a layer.
    pub const fn with_layer(id: StatId, layer: i32, value: i64) -> Stat {
        Stat { id, layer, value }
    }
}

/// Player stats section ("gf"), a list of stats.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct PlayerStats(pub Vec<Stat>);

impl PlayerStats {
    /// Section magic "gf".
    pub const MAGIC: u16 = 0x6667;

    /// Value of the first stat with the given ID, or 0.
    pub fn get(&self, id: StatId) -> i64 {
        self.0.iter().find(|s| s.id == id).map_or(0, |s| s.value)
    }

    /// Sets a stat (layer 0), replacing the first stat with the same ID or appending.
    pub fn set(&mut self, id: StatId, value: i64) {
        self.set_with_layer(id, 0, value);
    }

    /// Sets a stat with a layer, replacing the first stat with the same ID or appending.
    pub fn set_with_layer(&mut self, id: StatId, layer: i32, value: i64) {
        let stat = Stat { id, layer, value };
        match self.0.iter_mut().find(|s| s.id == id) {
            Some(existing) => *existing = stat,
            None => self.0.push(stat),
        }
    }

    /// Reads the player stats section.
    pub fn read(r: &mut BitReader<'_>, data: &(impl ExternalData + ?Sized), save_version: u32) -> Result<Self> {
        let magic = r.read_u16()?;
        if magic != Self::MAGIC {
            return Err(Error::InvalidMagic { section: "stats section", found: magic as u32 });
        }
        let mut stats = Vec::with_capacity(16);
        loop {
            let id = StatId(r.read_bits(9)? as u16);
            if id == StatId::TERMINATOR {
                break;
            }
            let info = data.stat_info(id, save_version)?;
            let layer = if info.csv_param > 0 { r.read_signed_bits(info.csv_param as u32)? } else { 0 };
            let value = if info.csv_bits < 32 && info.csv_signed {
                r.read_signed_bits(info.csv_bits as u32)? as i64
            } else {
                r.read_bits(info.csv_bits as u32)? as i64
            };
            stats.push(Stat { id, layer, value });
        }
        r.align_to_byte();
        Ok(PlayerStats(stats))
    }

    /// Writes the player stats section (values are clamped to their bit ranges).
    pub fn write(&self, w: &mut BitWriter<'_>, data: &(impl ExternalData + ?Sized), save_version: u32) -> Result<()> {
        w.write_u16(Self::MAGIC)?;
        for stat in &self.0 {
            let info = data.stat_info(stat.id, save_version)?;
            let bits = info.csv_bits as u32;
            let mut value = stat.value;
            if bits < 32 {
                if info.csv_signed {
                    let max = 1i64.wrapping_shl(bits.wrapping_sub(1));
                    let min = max.wrapping_neg();
                    value = value.clamp(min, max.wrapping_sub(1));
                } else {
                    value = value.clamp(0, 1i64.wrapping_shl(bits) - 1);
                }
            }
            w.write_bits(stat.id.0 as u32, 9)?;
            if info.csv_param > 0 {
                w.write_bits(stat.layer as u32, info.csv_param as u32)?;
            }
            if bits < 32 && info.csv_signed {
                w.write_signed_bits(value as i32, bits)?;
            } else {
                w.write_bits(value as u32, bits)?;
            }
        }
        w.write_bits(StatId::TERMINATOR.0 as u32, 9)?;
        w.align_to_byte();
        Ok(())
    }
}

impl Deref for PlayerStats {
    type Target = Vec<Stat>;
    fn deref(&self) -> &Vec<Stat> {
        &self.0
    }
}

impl DerefMut for PlayerStats {
    fn deref_mut(&mut self) -> &mut Vec<Stat> {
        &mut self.0
    }
}

impl<'a> IntoIterator for &'a PlayerStats {
    type Item = &'a Stat;
    type IntoIter = std::slice::Iter<'a, Stat>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl FromIterator<Stat> for PlayerStats {
    fn from_iter<T: IntoIterator<Item = Stat>>(iter: T) -> Self {
        PlayerStats(iter.into_iter().collect())
    }
}

/// Skills section ("if"): one level byte per class skill.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SkillsSection {
    /// The character class the skills belong to.
    pub class: CharacterClass,
    /// Skill levels indexed by the skill's offset within the class.
    pub levels: Vec<u8>,
}

impl SkillsSection {
    /// Section magic "if".
    pub const MAGIC: u16 = 0x6669;
    /// Number of skills per class.
    pub const SKILLS_PER_CLASS: usize = 30;
    /// First skill ID of each class.
    pub const CLASS_SKILL_OFFSETS: [u16; 8] = [6, 36, 66, 96, 126, 221, 251, 373];

    /// First skill ID of this section's class (`None` for unknown classes).
    pub fn skill_id_offset(&self) -> Option<u16> {
        Self::CLASS_SKILL_OFFSETS.get(self.class.to_raw() as usize).copied()
    }

    /// Level of the skill at `index` (0 if out of range).
    pub fn level(&self, index: usize) -> u8 {
        self.levels.get(index).copied().unwrap_or(0)
    }

    /// Sets the level of the skill at `index` (ignored if out of range).
    pub fn set_level(&mut self, index: usize, level: u8) {
        if let Some(l) = self.levels.get_mut(index) {
            *l = level;
        }
    }

    /// Skill ID for an index (0-29) within the class.
    pub fn skill_id(&self, index: usize) -> Option<u16> {
        if index >= Self::SKILLS_PER_CLASS {
            return None;
        }
        Some(self.skill_id_offset()? + index as u16)
    }

    /// Index (0-29) within the class for a skill ID.
    pub fn skill_index(&self, skill_id: u16) -> Option<usize> {
        let index = skill_id.checked_sub(self.skill_id_offset()?)? as usize;
        (index < Self::SKILLS_PER_CLASS).then_some(index)
    }

    /// Reads the skills section.
    pub fn read(r: &mut BitReader<'_>, class: CharacterClass, count: usize) -> Result<Self> {
        let magic = r.read_u16()?;
        if magic != Self::MAGIC {
            return Err(Error::InvalidMagic { section: "skills section", found: magic as u32 });
        }
        Ok(SkillsSection { class, levels: r.read_slice(count)?.to_vec() })
    }

    /// Writes the skills section.
    pub fn write(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.write_u16(Self::MAGIC)?;
        w.write_bytes(&self.levels)
    }
}

impl std::ops::Index<usize> for SkillsSection {
    type Output = u8;
    fn index(&self, index: usize) -> &u8 {
        &self.levels[index]
    }
}

impl std::ops::IndexMut<usize> for SkillsSection {
    fn index_mut(&mut self, index: usize) -> &mut u8 {
        &mut self.levels[index]
    }
}
