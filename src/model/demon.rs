use crate::bits::{BitReader, BitWriter};
use crate::data::ExternalData;
use crate::enums::{DemonType, Difficulty, GameVersion};
use crate::error::{Error, Result};
use crate::model::stats::{PlayerStats, Stat};
use crate::StatId;

/// Demon persistence section ("lf", v>=103).
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DemonSection {
    /// Section version (1).
    pub section_version: u16,
    /// Whether a demon exists.
    pub has_demon: bool,
    /// Padding byte after `has_demon` (always written as 0 by the game).
    pub padding: u8,
    /// Demon data (present when `has_demon`).
    pub demon: Option<DemonData>,
}

impl Default for DemonSection {
    fn default() -> Self {
        DemonSection { section_version: 1, has_demon: false, padding: 0, demon: None }
    }
}

impl DemonSection {
    /// Section magic "lf".
    pub const MAGIC: u16 = 0x666C;

    /// Reads the section if present (returns `None` without consuming input otherwise).
    pub fn try_read(
        r: &mut BitReader<'_>,
        data: &(impl ExternalData + ?Sized),
        save_version: u32,
    ) -> Result<Option<Self>> {
        if save_version < 103 || r.bits_remaining() < 48 {
            return Ok(None);
        }
        let start = r.bit_position();
        let version = r.read_u16()?;
        let magic = r.read_u16()?;
        if magic != Self::MAGIC || version != 1 {
            r.set_bit_position(start);
            return Ok(None);
        }
        let has_demon = r.read_u8()? != 0;
        let padding = r.read_u8()?;
        let demon = if has_demon { Some(DemonData::read(r, data, save_version)?) } else { None };
        Ok(Some(DemonSection { section_version: version, has_demon, padding, demon }))
    }

    /// Writes the section.
    pub fn write(&self, w: &mut BitWriter<'_>, data: &(impl ExternalData + ?Sized), save_version: u32) -> Result<()> {
        w.write_u16(self.section_version)?;
        w.write_u16(Self::MAGIC)?;
        w.write_u8(self.has_demon as u8)?;
        w.write_u8(self.padding)?;
        if let (true, Some(demon)) = (self.has_demon, &self.demon) {
            demon.write(w, data, save_version)?;
        }
        Ok(())
    }
}

/// A bound demon: a 92-byte descriptor followed by a "gf" stat list with 32-bit raw values.
///
/// Offsets 0x0C..0x43 mirror the game's monster init data, used on load to recreate the
/// monster with its original spawn parameters. Padding fields are never written by the game
/// and contain uninitialized data; they are preserved for byte-exact round-trips.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DemonData {
    /// Demon type.
    pub demon_type: DemonType,
    /// Monster class row (0-based; stored 1-based). A MonStats.txt row for standard
    /// demons, a SuperUniques.txt row for super uniques.
    pub class_id: u16,
    /// Monster name seed.
    pub name_seed: u16,
    /// Monster type flags (SuperUnique 0x02, Champion 0x04, Unique 0x08, Minion 0x10,
    /// empowered boss 0x200, empowered boss minion 0x400).
    pub mon_flags: u16,
    /// Stat count of the legacy (v<=94) stat format; the game always writes 16.
    pub num_stats: u16,
    /// Difficulty of the game the demon was spawned in.
    pub difficulty: Difficulty,
    /// Game version of the game the demon was spawned in.
    pub game_version: GameVersion,
    /// Struct padding.
    pub padding_0e: u16,
    /// Non-zero if the game the demon was spawned in was a ladder game.
    pub is_ladder: u32,
    /// Game type of the game the demon was spawned in (0 = single player, 3 = Battle.net, ...).
    /// When non-zero, or when `is_ladder` is non-zero, monster stats use the online MonLvl columns.
    pub game_type: u8,
    /// Struct padding.
    pub padding_15: [u8; 3],
    /// Levels.txt ID of the area the demon originally spawned in.
    pub level_id: u32,
    /// Monster level used for stat calculation.
    pub monster_level: u8,
    /// Non-zero if the demon spawned in a terrorized area.
    pub is_terrorized: u8,
    /// Struct padding.
    pub padding_1e: u16,
    /// Terror zone parameter of the spawn area.
    pub terror_zone_param: u32,
    /// Bonus added to `monster_level` when the monster is recreated.
    pub level_bonus: u32,
    /// Bonus damage percent for empowered bosses and their minions.
    pub bonus_damage_percent: u32,
    /// Player count HP percent bonus.
    pub player_count_hp_bonus: i32,
    /// Player count experience percent bonus.
    pub player_count_experience_bonus: i32,
    /// Player count monster skill level bonus.
    pub player_count_skill_bonus: i32,
    /// Player count bonus difficulty.
    pub player_count_difficulty: i32,
    /// Player count (minimum 1).
    pub player_count: i32,
    /// Monster graphic components.
    pub components: [u8; 16],
    /// Monster unique modifiers (valid mods packed to the front).
    pub mon_umod: [u8; 9],
    /// Struct padding.
    pub padding_59: [u8; 3],
    /// Stats (raw 32-bit values, no SaveAdd/ValShift transform).
    pub stats: Vec<Stat>,
}

impl Default for DemonData {
    fn default() -> Self {
        DemonData {
            demon_type: DemonType::default(),
            class_id: 0,
            name_seed: 0,
            mon_flags: 0,
            num_stats: 16,
            difficulty: Difficulty::default(),
            game_version: GameVersion::default(),
            padding_0e: 0,
            is_ladder: 0,
            game_type: 0,
            padding_15: [0; 3],
            level_id: 0,
            monster_level: 0,
            is_terrorized: 0,
            padding_1e: 0,
            terror_zone_param: 0,
            level_bonus: 0,
            bonus_damage_percent: 0,
            player_count_hp_bonus: 0,
            player_count_experience_bonus: 0,
            player_count_skill_bonus: 0,
            player_count_difficulty: 0,
            player_count: 0,
            components: [0; 16],
            mon_umod: [0; 9],
            padding_59: [0; 3],
            stats: Vec::new(),
        }
    }
}

impl DemonData {
    const MAX_STAT_BLOB: usize = 16384;

    /// Reads the demon descriptor and stats.
    pub fn read(r: &mut BitReader<'_>, data: &(impl ExternalData + ?Sized), save_version: u32) -> Result<Self> {
        let stat_size = r.read_u16()? as usize;
        let mut demon = DemonData {
            demon_type: DemonType::from_raw(r.read_u16()?),
            class_id: r.read_u16()?.wrapping_sub(1),
            name_seed: r.read_u16()?,
            mon_flags: r.read_u16()?,
            num_stats: r.read_u16()?,
            difficulty: Difficulty::from_raw(r.read_u8()?),
            game_version: GameVersion::from_raw(r.read_u8()?),
            padding_0e: r.read_u16()?,
            is_ladder: r.read_u32()?,
            game_type: r.read_u8()?,
            padding_15: r.read_array()?,
            level_id: r.read_u32()?,
            monster_level: r.read_u8()?,
            is_terrorized: r.read_u8()?,
            padding_1e: r.read_u16()?,
            terror_zone_param: r.read_u32()?,
            level_bonus: r.read_u32()?,
            bonus_damage_percent: r.read_u32()?,
            player_count_hp_bonus: r.read_u32()? as i32,
            player_count_experience_bonus: r.read_u32()? as i32,
            player_count_skill_bonus: r.read_u32()? as i32,
            player_count_difficulty: r.read_u32()? as i32,
            player_count: r.read_u32()? as i32,
            components: r.read_array()?,
            mon_umod: r.read_array()?,
            padding_59: r.read_array()?,
            stats: Vec::new(),
        };
        if stat_size > 0 {
            let blob = r.read_slice(stat_size)?;
            demon.stats = read_stats(blob, data, save_version)?;
        }
        Ok(demon)
    }

    /// Writes the demon descriptor and stats.
    pub fn write(&self, w: &mut BitWriter<'_>, data: &(impl ExternalData + ?Sized), save_version: u32) -> Result<()> {
        let mut blob = Vec::with_capacity(4 + self.stats.len() * 6);
        write_stats(&mut blob, &self.stats, data, save_version)?;
        if blob.len() > Self::MAX_STAT_BLOB {
            return Err(Error::invalid_data("demon stat data too large"));
        }
        w.write_u16(blob.len() as u16)?;
        w.write_u16(self.demon_type.to_raw())?;
        w.write_u16(self.class_id.wrapping_add(1))?;
        w.write_u16(self.name_seed)?;
        w.write_u16(self.mon_flags)?;
        w.write_u16(self.num_stats)?;
        w.write_u8(self.difficulty.to_raw())?;
        w.write_u8(self.game_version.to_raw())?;
        w.write_u16(self.padding_0e)?;
        w.write_u32(self.is_ladder)?;
        w.write_u8(self.game_type)?;
        w.write_bytes(&self.padding_15)?;
        w.write_u32(self.level_id)?;
        w.write_u8(self.monster_level)?;
        w.write_u8(self.is_terrorized)?;
        w.write_u16(self.padding_1e)?;
        w.write_u32(self.terror_zone_param)?;
        w.write_u32(self.level_bonus)?;
        w.write_u32(self.bonus_damage_percent)?;
        w.write_u32(self.player_count_hp_bonus as u32)?;
        w.write_u32(self.player_count_experience_bonus as u32)?;
        w.write_u32(self.player_count_skill_bonus as u32)?;
        w.write_u32(self.player_count_difficulty as u32)?;
        w.write_u32(self.player_count as u32)?;
        w.write_bytes(&self.components)?;
        w.write_bytes(&self.mon_umod)?;
        w.write_bytes(&self.padding_59)?;
        w.write_bytes(&blob)
    }
}

fn read_stats(blob: &[u8], data: &(impl ExternalData + ?Sized), save_version: u32) -> Result<Vec<Stat>> {
    let mut r = BitReader::new(blob);
    let magic = r.read_u16()?;
    if magic != PlayerStats::MAGIC {
        return Err(Error::InvalidMagic { section: "demon stats", found: magic as u32 });
    }
    let mut stats = Vec::new();
    loop {
        let id = StatId(r.read_bits(9)? as u16);
        if id == StatId::TERMINATOR {
            break;
        }
        let info = data.stat_info(id, save_version)?;
        let layer = if info.csv_param > 0 { r.read_signed_bits(info.csv_param as u32)? } else { 0 };
        stats.push(Stat { id, layer, value: r.read_u32()? as i64 });
    }
    Ok(stats)
}

fn write_stats(
    buf: &mut Vec<u8>,
    stats: &[Stat],
    data: &(impl ExternalData + ?Sized),
    save_version: u32,
) -> Result<()> {
    let mut w = BitWriter::new(buf);
    w.write_u16(PlayerStats::MAGIC)?;
    for stat in stats {
        let info = data.stat_info(stat.id, save_version)?;
        w.write_bits(stat.id.0 as u32, 9)?;
        if info.csv_param > 0 {
            w.write_bits(stat.layer as u32, info.csv_param as u32)?;
        }
        let value = u32::try_from(stat.value)
            .map_err(|_| Error::invalid_data(format!("demon stat value {} out of range", stat.value)))?;
        w.write_u32(value)?;
    }
    w.write_bits(StatId::TERMINATOR.0 as u32, 9)?;
    w.align_to_byte();
    Ok(())
}
