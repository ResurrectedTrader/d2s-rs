use crate::bits::{BitReader, BitWriter};
use crate::enums::{CharacterClass, CharacterFlags, GameVersion, ItemFlags, ItemQuality, Side};
use crate::error::Result;
use crate::ItemCode;

/// Character section. 319 bytes for v<=103, 387 bytes for v>=104.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Character {
    /// Active weapon set.
    pub weapon_switch: u32,
    /// Character name (max 15 chars). Only serialized for v<=103.
    pub name: String,
    /// Character save flags.
    pub flags: CharacterFlags,
    /// Character class.
    pub class: CharacterClass,
    /// Number of stats (always 16).
    pub num_stats: u8,
    /// Number of skills per class; determines the size of the skills section.
    pub num_skills: u8,
    /// Character level.
    pub level: u8,
    /// Creation timestamp.
    pub create_time: u32,
    /// Last save timestamp.
    pub last_save_time: u32,
    /// Total play time.
    pub play_time: u32,
    /// Skill hotkey assignments.
    pub skill_hotkeys: [Skill; 16],
    /// Left mouse button skill.
    pub left_skill: Skill,
    /// Right mouse button skill.
    pub right_skill: Skill,
    /// Weapon-switch left mouse button skill.
    pub switch_left_skill: Skill,
    /// Weapon-switch right mouse button skill.
    pub switch_right_skill: Skill,
    /// Appearance graphics per composit.
    pub appearance_graphics: [u8; 16],
    /// Appearance tints per composit.
    pub appearance_tints: [u8; 16],
    /// Current town per difficulty.
    pub town_difficulty: [u8; 3],
    /// Map random seed.
    pub map_seed: u32,
    /// Mercenary data.
    pub merc: MercData,
    /// Character selection screen preview data (holds the UTF-8 name for D2R).
    pub preview: PreviewData,
}

impl Default for Character {
    fn default() -> Self {
        Character {
            weapon_switch: 0,
            name: String::new(),
            flags: CharacterFlags::empty(),
            class: CharacterClass::Amazon,
            num_stats: 16,
            num_skills: 30,
            level: 1,
            create_time: 0,
            last_save_time: 0,
            play_time: 0xFFFF_FFFF,
            skill_hotkeys: Default::default(),
            left_skill: Skill::default(),
            right_skill: Skill::default(),
            switch_left_skill: Skill::default(),
            switch_right_skill: Skill::default(),
            appearance_graphics: [0xFF; 16],
            appearance_tints: [0xFF; 16],
            town_difficulty: [0; 3],
            map_seed: 0,
            merc: MercData::default(),
            preview: PreviewData::default(),
        }
    }
}

impl Character {
    /// Section size for v<=103.
    pub const SIZE_V103: usize = 319;
    /// Section size for v>=104.
    pub const SIZE_V104: usize = 387;

    /// Section size for a save version.
    pub fn size(save_version: u32) -> usize {
        if save_version >= 104 {
            Self::SIZE_V104
        } else {
            Self::SIZE_V103
        }
    }

    /// Reads the character section.
    pub fn read(r: &mut BitReader<'_>, save_version: u32) -> Result<Self> {
        let weapon_switch = r.read_u32()?;
        let name = if save_version < 104 { r.read_fixed_string(16)? } else { String::new() };
        let flags = CharacterFlags::from_bits_retain(r.read_u32()?);
        let class = CharacterClass::from_raw(r.read_u8()?);
        let num_stats = r.read_u8()?;
        let num_skills = r.read_u8()?;
        let level = r.read_u8()?;
        let create_time = r.read_u32()?;
        let last_save_time = r.read_u32()?;
        let play_time = r.read_u32()?;
        let mut skill_hotkeys = <[Skill; 16]>::default();
        for hotkey in &mut skill_hotkeys {
            *hotkey = Skill::read(r)?;
        }
        Ok(Character {
            weapon_switch,
            name,
            flags,
            class,
            num_stats,
            num_skills,
            level,
            create_time,
            last_save_time,
            play_time,
            skill_hotkeys,
            left_skill: Skill::read(r)?,
            right_skill: Skill::read(r)?,
            switch_left_skill: Skill::read(r)?,
            switch_right_skill: Skill::read(r)?,
            appearance_graphics: r.read_array()?,
            appearance_tints: r.read_array()?,
            town_difficulty: r.read_array()?,
            map_seed: r.read_u32()?,
            merc: MercData::read(r)?,
            preview: PreviewData::read(r, save_version)?,
        })
    }

    /// Writes the character section.
    pub fn write(&self, w: &mut BitWriter<'_>, save_version: u32) -> Result<()> {
        w.write_u32(self.weapon_switch)?;
        if save_version < 104 {
            w.write_fixed_string(&self.name, 16)?;
        }
        w.write_u32(self.flags.bits())?;
        w.write_u8(self.class.to_raw())?;
        w.write_u8(self.num_stats)?;
        w.write_u8(self.num_skills)?;
        w.write_u8(self.level)?;
        w.write_u32(self.create_time)?;
        w.write_u32(self.last_save_time)?;
        w.write_u32(self.play_time)?;
        for hotkey in &self.skill_hotkeys {
            hotkey.write(w)?;
        }
        self.left_skill.write(w)?;
        self.right_skill.write(w)?;
        self.switch_left_skill.write(w)?;
        self.switch_right_skill.write(w)?;
        w.write_bytes(&self.appearance_graphics)?;
        w.write_bytes(&self.appearance_tints)?;
        w.write_bytes(&self.town_difficulty)?;
        w.write_u32(self.map_seed)?;
        self.merc.write(w)?;
        self.preview.write(w, save_version)
    }
}

/// Skill hotkey assignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Skill {
    /// Skill ID (15 bits).
    pub skill_id: u16,
    /// Skill bar side.
    pub side: Side,
    /// Index of the item granting the skill.
    pub item_index: u16,
}

impl Skill {
    /// Reads a skill assignment.
    pub fn read(r: &mut BitReader<'_>) -> Result<Self> {
        let raw = r.read_u16()?;
        Ok(Skill {
            skill_id: raw & 0x7FFF,
            side: if raw >> 15 != 0 { Side::Left } else { Side::Right },
            item_index: r.read_u16()?,
        })
    }

    /// Writes a skill assignment.
    pub fn write(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.write_u16((self.skill_id & 0x7FFF) | ((self.side as u16) << 15))?;
        w.write_u16(self.item_index)
    }
}

/// Mercenary data in the character header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MercData {
    /// Mercenary flags (only bit 16 = dead is used).
    pub flags: u32,
    /// Random seed.
    pub seed: u32,
    /// Name index relative to the hireling type.
    pub name_index: u16,
    /// Hireling type ID.
    pub hireling_id: u16,
    /// Experience points.
    pub experience: u32,
}

impl MercData {
    const DEAD: u32 = 0x0001_0000;

    /// Whether the mercenary is dead.
    pub fn is_dead(&self) -> bool {
        self.flags & Self::DEAD != 0
    }

    /// Sets whether the mercenary is dead.
    pub fn set_dead(&mut self, dead: bool) {
        if dead {
            self.flags |= Self::DEAD;
        } else {
            self.flags &= !Self::DEAD;
        }
    }

    /// Whether a mercenary has been hired.
    pub fn has_merc(&self) -> bool {
        self.seed != 0 || self.name_index != 0 || self.experience != 0
    }

    /// Reads mercenary data.
    pub fn read(r: &mut BitReader<'_>) -> Result<Self> {
        Ok(MercData {
            flags: r.read_u32()?,
            seed: r.read_u32()?,
            name_index: r.read_u16()?,
            hireling_id: r.read_u16()?,
            experience: r.read_u32()?,
        })
    }

    /// Writes mercenary data.
    pub fn write(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.write_u32(self.flags)?;
        w.write_u32(self.seed)?;
        w.write_u16(self.name_index)?;
        w.write_u16(self.hireling_id)?;
        w.write_u32(self.experience)
    }
}

/// Character selection screen preview data.
///
/// v<=103 (144 bytes): two save times, guild emblem color, game version, padding, two
/// experiences, four preview items, 60-byte name. v>=104 (228 bytes): six save times,
/// six experiences, guild emblem color, game version, padding, four preview items,
/// 96-byte name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PreviewData {
    /// Ladder save timestamps (Windows FILETIME), one slot per game version:
    /// `[0]` expansion, `[1]` classic, `[2]` Reign of the Warlock. The client only updates
    /// these three (in ladder games); `[3..5]` are preserved. v<=103 stores `[0]` and `[1]`.
    pub save_times: [u64; 6],
    /// Ladder experience, indexed like `save_times`. v<=103 stores `[0]` and `[1]`.
    pub experiences: [u32; 6],
    /// Guild emblem background color. The game only accepts 0 or 0xFF; 0xFF on a
    /// ladder character lets it load into a non-ladder game.
    pub guild_emblem_color: u8,
    /// Game version. For v<=103 the game falls back to the expansion character flag
    /// when this is not 1-3.
    pub game_version: GameVersion,
    /// Padding after the game version (never written by the game).
    pub preview_padding: u16,
    /// Left hand preview item.
    pub left_hand: PreviewItem,
    /// Right hand preview item.
    pub right_hand: PreviewItem,
    /// Torso preview item.
    pub torso: PreviewItem,
    /// Head preview item.
    pub head: PreviewItem,
    /// Character name (UTF-8). 60 bytes for v<=103, 96 bytes for v>=104.
    pub name: String,
    /// Reserved bytes after the name (always written as zero, never read by the game).
    pub reserved: u64,
}

impl PreviewData {
    /// Expansion save time (`save_times[0]`).
    pub fn expansion_save_time(&self) -> u64 {
        self.save_times[0]
    }

    /// Classic save time (`save_times[1]`).
    pub fn classic_save_time(&self) -> u64 {
        self.save_times[1]
    }

    /// Expansion experience (`experiences[0]`).
    pub fn expansion_experience(&self) -> u32 {
        self.experiences[0]
    }

    /// Classic experience (`experiences[1]`).
    pub fn classic_experience(&self) -> u32 {
        self.experiences[1]
    }

    /// Reads preview data.
    pub fn read(r: &mut BitReader<'_>, save_version: u32) -> Result<Self> {
        let mut data = PreviewData::default();
        if save_version >= 104 {
            for t in &mut data.save_times {
                *t = r.read_u64()?;
            }
            for e in &mut data.experiences {
                *e = r.read_u32()?;
            }
            data.read_emblem_and_version(r)?;
        } else {
            data.save_times[0] = r.read_u64()?;
            data.save_times[1] = r.read_u64()?;
            data.read_emblem_and_version(r)?;
            data.experiences[0] = r.read_u32()?;
            data.experiences[1] = r.read_u32()?;
        }
        data.left_hand = PreviewItem::read(r)?;
        data.right_hand = PreviewItem::read(r)?;
        data.torso = PreviewItem::read(r)?;
        data.head = PreviewItem::read(r)?;
        data.name = r.read_fixed_string(Self::name_size(save_version))?;
        data.reserved = r.read_u64()?;
        Ok(data)
    }

    fn read_emblem_and_version(&mut self, r: &mut BitReader<'_>) -> Result<()> {
        self.guild_emblem_color = r.read_u8()?;
        self.game_version = GameVersion::from_raw(r.read_u8()?);
        self.preview_padding = r.read_u16()?;
        Ok(())
    }

    fn write_emblem_and_version(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.write_u8(self.guild_emblem_color)?;
        w.write_u8(self.game_version.to_raw())?;
        w.write_u16(self.preview_padding)
    }

    /// Writes preview data.
    pub fn write(&self, w: &mut BitWriter<'_>, save_version: u32) -> Result<()> {
        if save_version >= 104 {
            for &t in &self.save_times {
                w.write_u64(t)?;
            }
            for &e in &self.experiences {
                w.write_u32(e)?;
            }
            self.write_emblem_and_version(w)?;
        } else {
            w.write_u64(self.save_times[0])?;
            w.write_u64(self.save_times[1])?;
            self.write_emblem_and_version(w)?;
            w.write_u32(self.experiences[0])?;
            w.write_u32(self.experiences[1])?;
        }
        self.left_hand.write(w)?;
        self.right_hand.write(w)?;
        self.torso.write(w)?;
        self.head.write(w)?;
        w.write_fixed_string(&self.name, Self::name_size(save_version))?;
        w.write_u64(self.reserved)
    }

    fn name_size(save_version: u32) -> usize {
        if save_version >= 104 {
            96
        } else {
            60
        }
    }
}

/// Compact description of an equipped item for the character selection screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PreviewItem {
    /// Item code.
    pub code: ItemCode,
    /// Color transform index.
    pub transform: u8,
    /// Item quality.
    pub quality: ItemQuality,
    /// Graphics file index.
    pub file_index: u16,
    /// Item flags.
    pub flags: ItemFlags,
}

impl PreviewItem {
    /// Reads a preview item.
    pub fn read(r: &mut BitReader<'_>) -> Result<Self> {
        Ok(PreviewItem {
            code: ItemCode(r.read_u32()?),
            transform: r.read_u8()?,
            quality: ItemQuality::from_raw(r.read_u8()?),
            file_index: r.read_u16()?,
            flags: ItemFlags::from_bits_retain(r.read_u32()?),
        })
    }

    /// Writes a preview item.
    pub fn write(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.write_u32(self.code.0)?;
        w.write_u8(self.transform)?;
        w.write_u8(self.quality.to_raw())?;
        w.write_u16(self.file_index)?;
        w.write_u32(self.flags.bits())
    }
}
