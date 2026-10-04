use crate::bits::{BitReader, BitWriter};
use crate::data::{ExternalData, ItemTypeInfo};
use crate::enums::{BodyLocation, Difficulty, ItemFlags, ItemMode, ItemQuality, StorePage};
use crate::error::{Error, Result};
use crate::model::quality::QualityData;
use crate::model::stats::Stat;
use crate::{huffman, ItemCode, StatId};

/// Item position/location.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ItemPosition {
    /// Animation/storage mode.
    pub mode: ItemMode,
    /// World X position (on ground).
    pub ground_x: u16,
    /// World Y position (on ground).
    pub ground_y: u16,
    /// Body location (when equipped).
    pub body_location: BodyLocation,
    /// Inventory X position (0-15).
    pub inv_x: u8,
    /// Inventory Y position (0-15).
    pub inv_y: u8,
    /// Storage page.
    pub store_page: StorePage,
}

impl ItemPosition {
    /// Whether the item is on the ground (or dropping).
    pub fn is_on_ground(&self) -> bool {
        matches!(self.mode, ItemMode::OnGround | ItemMode::Dropping)
    }

    /// Reads a position for the given mode.
    pub fn read(r: &mut BitReader<'_>, mode: ItemMode) -> Result<Self> {
        let mut pos = ItemPosition { mode, ..Default::default() };
        if pos.is_on_ground() {
            pos.ground_x = r.read_u16()?;
            pos.ground_y = r.read_u16()?;
        } else {
            pos.body_location = BodyLocation::from_raw(r.read_bits(4)? as u8);
            pos.inv_x = r.read_bits(4)? as u8;
            pos.inv_y = r.read_bits(4)? as u8;
            // The file stores page + 1; raw 0 wraps to Null.
            pos.store_page = StorePage::from_raw((r.read_bits(3)? as u8).wrapping_sub(1));
        }
        Ok(pos)
    }

    /// Writes the position (not including the mode).
    pub fn write(&self, w: &mut BitWriter<'_>) -> Result<()> {
        if self.is_on_ground() {
            w.write_u16(self.ground_x)?;
            w.write_u16(self.ground_y)
        } else {
            w.write_bits(self.body_location.to_raw() as u32, 4)?;
            w.write_bits(self.inv_x.min(15) as u32, 4)?;
            w.write_bits(self.inv_y.min(15) as u32, 4)?;
            let raw_page = match self.store_page {
                StorePage::Null => 0,
                page => (page.to_raw() as u32 + 1).min(7),
            };
            w.write_bits(raw_page, 3)
        }
    }
}

/// A single item.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Item {
    /// Item flags.
    pub flags: ItemFlags,
    /// Item format version.
    pub version: u16,
    /// Item position.
    pub position: ItemPosition,
    /// Item code.
    pub code: ItemCode,

    /// Ear owner's class (ear items).
    pub ear_file_index: u8,
    /// Ear owner's level (ear items).
    pub ear_level: u8,
    /// Ear owner's name (ear items).
    pub ear_name: String,

    /// Gold amount (gold items).
    pub gold_amount: u32,
    /// Player gold flag (gold items, D2R).
    pub player_gold: bool,

    /// Difficulty the quest item was found in (compact quest items with a difficulty check).
    pub quest_difficulty: Option<Difficulty>,

    /// Random seed.
    pub seed: u32,
    /// Item level.
    pub level: u8,
    /// Item quality.
    pub quality: ItemQuality,
    /// Variable graphics index.
    pub variable_gfx_id: Option<u8>,
    /// Auto affix ID.
    pub auto_affix_id: Option<u16>,
    /// Quality specific data.
    pub quality_data: Option<QualityData>,
    /// Runeword ID.
    pub runeword_id: Option<u16>,
    /// Personalized name.
    pub personalized_name: String,
    /// Opaque 128-bit realm data, never generated or interpreted by the client.
    /// Version > 96 stores 4 values; versions 87-96 store 2 (plus one discarded value for
    /// 94-96), which the game maps to elements `[2]` and `[3]` of the 128-bit value.
    pub realm_data: Option<Vec<u32>>,

    /// Defense (armor).
    pub defense: Option<i32>,
    /// Maximum durability.
    pub max_durability: Option<i32>,
    /// Current durability.
    pub durability: Option<i32>,
    /// Stack quantity.
    pub quantity: Option<i32>,
    /// Set bonus mask (set items).
    pub set_item_mask: Option<u8>,

    /// Item stats.
    pub stats: Vec<Stat>,
    /// Set bonus stat lists, one per bit set in `set_item_mask`.
    pub set_bonus_stats: Vec<Vec<Stat>>,
    /// Runeword stats.
    pub runeword_stats: Option<Vec<Stat>>,

    /// Sockets: filled sockets are `Some`, empty sockets `None`.
    pub sockets: Vec<Option<Item>>,

    /// String table ID of the name of whatever dropped the item (v100+, chronicle flag);
    /// becomes [`ChronicleEntry::source`](crate::ChronicleEntry::source) when recorded.
    pub chronicle_id: Option<u16>,
    /// When the item dropped, in minutes since the Unix epoch (v100+, non-compact chronicle).
    pub chronicle_timestamp: Option<u32>,
    /// 64-bit game account IDs receiving the chronicle entry when identified (up to 8, v100+).
    pub chronicle_recipients: Option<Vec<u64>>,

    /// Stack size in advanced stash tabs (v100+).
    pub advanced_stash_stack_size: Option<u8>,
}

/// Item codes that carry an advanced stash stack size in v100-101.
const STACKABLE_CODES: &str =
    "rvs rvl gcv gfv gsv gzv gpv gcy gfy gsy gly gpy gcb gfb gsb glb gpb gcg gfg gsg glg gpg gcr gfr gsr glr gpr gcw \
gfw gsw glw gpw skc skf sku skl skz r01 r02 r03 r04 r05 r06 r07 r08 r09 r10 r11 r12 r13 r14 r15 r16 r17 r18 r19 \
r20 r21 r22 r23 r24 r25 r26 r27 r28 r29 r30 r31 r32 r33 pk1 pk2 pk3 dhn bey mbr toa tes ceh bet fed ";

fn is_v100_stackable(code: ItemCode) -> bool {
    STACKABLE_CODES.as_bytes().chunks_exact(4).any(|c| u32::from_le_bytes(c.try_into().unwrap()) == code.0)
}

impl Item {
    /// Item magic "JM" (v96 only).
    pub const MAGIC: u16 = 0x4D4A;

    /// Creates an item with the given code.
    pub fn new(code: impl Into<ItemCode>) -> Item {
        Item { code: code.into(), ..Default::default() }
    }

    /// Filled sockets.
    pub fn socketed_items(&self) -> impl Iterator<Item = &Item> {
        self.sockets.iter().flatten()
    }

    /// Mutable filled sockets.
    pub fn socketed_items_mut(&mut self) -> impl Iterator<Item = &mut Item> {
        self.sockets.iter_mut().flatten()
    }

    /// Item code as a trimmed string.
    pub fn code_string(&self) -> String {
        self.code.as_string()
    }

    /// Number of items including socketed items (recursively).
    pub fn count_recursive(&self) -> usize {
        1 + self.socketed_items().map(Item::count_recursive).sum::<usize>()
    }

    /// Reads an item (and its socketed items).
    pub fn read(r: &mut BitReader<'_>, data: &(impl ExternalData + ?Sized), save_version: u32) -> Result<Item> {
        let mut item = Item::default();
        if save_version <= 96 {
            let magic = r.read_u16()?;
            if magic != Self::MAGIC {
                return Err(Error::InvalidMagic { section: "item", found: magic as u32 });
            }
        }

        let mut flags = ItemFlags::from_bits_retain(r.read_u32()?);
        let is_gamble = flags.contains(ItemFlags::LOW_QUALITY);
        flags.remove(ItemFlags::LOW_QUALITY | ItemFlags::INIT);
        item.flags = flags;

        let result = if flags.contains(ItemFlags::COMPACT_SAVE) {
            item.read_compact(r, data, save_version).map(|()| 0)
        } else {
            item.read_complete(r, data, is_gamble, save_version)
        };
        let filled = match result {
            Ok(filled) => filled,
            Err(e @ Error::Item { .. }) => return Err(e),
            Err(e) => {
                return Err(Error::Item {
                    code: item.code,
                    flags: item.flags.bits(),
                    compact: flags.contains(ItemFlags::COMPACT_SAVE),
                    quality: item.quality.to_raw(),
                    bit_position: r.bit_position(),
                    source: Box::new(e),
                })
            }
        };

        // Every item is serialized into its own byte buffer, so items are byte aligned.
        r.align_to_byte();

        for slot in item.sockets.iter_mut().take(filled) {
            *slot = Some(Item::read(r, data, save_version)?);
        }
        Ok(item)
    }

    fn read_compact(&mut self, r: &mut BitReader<'_>, data: &(impl ExternalData + ?Sized), v: u32) -> Result<()> {
        self.version = read_item_format(r, v)?;
        let mode = ItemMode::from_raw(r.read_bits(3)? as u8);
        self.position = ItemPosition::read(r, mode)?;

        if self.flags.contains(ItemFlags::IS_EAR) {
            self.code = ItemCode::EAR;
            self.ear_file_index = r.read_bits(3)? as u8;
            self.ear_level = r.read_bits(7)? as u8;
            self.ear_name = read_string(r, v)?;
        } else {
            self.code = read_item_code(r, v)?;
            if data.item_info(self.code, v)?.is_gold {
                self.read_gold(r, v)?;
            }
        }

        let info = data.item_info(self.code, v)?;
        if v > 92 && info.is_quest && info.quest_diff_check {
            let stat = data.stat_info(StatId::QUEST_ITEM_DIFFICULTY, v)?;
            let raw = r.read_bits(stat.save_bits as u32)? as i32;
            let value = raw.wrapping_sub(stat.save_add).wrapping_shl(stat.val_shift as u32);
            self.quest_difficulty = Some(Difficulty::from_raw(value as u8));
        }

        self.read_realm_data(r, v)?;
        self.read_advanced_stash_stack_size(r, v)?;

        self.level = 1;
        self.quality = ItemQuality::Normal;
        Ok(())
    }

    /// Returns the number of filled sockets that follow the item.
    fn read_complete(
        &mut self,
        r: &mut BitReader<'_>,
        data: &(impl ExternalData + ?Sized),
        is_gamble: bool,
        v: u32,
    ) -> Result<usize> {
        self.version = read_item_format(r, v)?;
        let mode = ItemMode::from_raw(r.read_bits(3)? as u8);
        self.position = ItemPosition::read(r, mode)?;
        self.code = read_item_code(r, v)?;

        if is_gamble {
            self.level = 1;
            self.quality = ItemQuality::Inferior;
            return Ok(0);
        }

        let filled = r.read_bits(3)? as usize;
        self.sockets = vec![None; filled];
        self.seed = r.read_u32()?;
        self.level = (r.read_bits(7)? as u8).max(1);
        self.quality = ItemQuality::from_raw(r.read_bits(4)? as u8);

        if r.read_bool()? {
            self.variable_gfx_id = Some(r.read_bits(3)? as u8);
        }
        if r.read_bool()? {
            let raw = r.read_bits(11)? as i32;
            self.auto_affix_id = Some(raw.wrapping_add(data.auto_magic_offset(v)) as u16);
        }

        let info = data.item_info(self.code, v)?;
        self.quality_data = read_quality_data(r, self.quality, &info, data.magic_prefix_offset(v))?;

        if self.flags.contains(ItemFlags::RUNEWORD) {
            self.runeword_id = Some(r.read_u16()?);
        }

        if self.flags.contains(ItemFlags::IS_EAR) {
            self.ear_file_index = r.read_bits(3)? as u8;
            self.ear_level = r.read_bits(7)? as u8;
            self.ear_name = read_string(r, v)?;
        } else if self.flags.contains(ItemFlags::PERSONALIZED) {
            self.personalized_name = read_string(r, v)?;
        }

        self.read_realm_data(r, v)?;

        if info.is_armor {
            let ac = data.stat_info(StatId::ARMOR_CLASS, v)?;
            self.defense = Some((r.read_bits(ac.save_bits as u32)? as i32).wrapping_sub(ac.save_add));
            self.read_durability(r, data, v)?;
        } else if info.is_weapon {
            self.read_durability(r, data, v)?;
        } else if info.is_gold {
            self.read_gold(r, v)?;
        }

        if v > 104 {
            if r.read_bool()? {
                self.quantity = Some(r.read_bits(9)? as i32);
            }
        } else if info.is_stackable {
            self.quantity = Some(r.read_bits(data.quantity_bits(v))? as i32);
        }

        if self.flags.contains(ItemFlags::SOCKETED) {
            let stat = data.stat_info(StatId::NUM_SOCKETS, v)?;
            let count = r.read_bits(stat.save_bits as u32)? as usize;
            if count < self.sockets.len() {
                return Err(Error::invalid_data(format!(
                    "socket count {count} is less than the number of socketed items {}",
                    self.sockets.len()
                )));
            }
            self.sockets.resize(count, None);
        }

        if self.quality == ItemQuality::Set {
            self.set_item_mask = Some(r.read_bits(5)? as u8);
        }

        self.stats = read_stat_list(r, data, v)?;

        if let (ItemQuality::Set, Some(mask)) = (self.quality, self.set_item_mask) {
            for i in 0..5 {
                if mask & (1 << i) != 0 {
                    self.set_bonus_stats.push(read_stat_list(r, data, v)?);
                }
            }
        }

        if self.flags.contains(ItemFlags::RUNEWORD) {
            self.runeword_stats = Some(read_stat_list(r, data, v)?);
        }

        self.read_chronicle(r, v)?;
        self.read_advanced_stash_stack_size(r, v)?;
        Ok(filled)
    }

    fn read_gold(&mut self, r: &mut BitReader<'_>, v: u32) -> Result<()> {
        let big = r.read_bool()?;
        self.gold_amount = if big { r.read_u32()? } else { r.read_bits(12)? };
        if v > 96 {
            self.player_gold = r.read_bool()?;
        }
        Ok(())
    }

    fn read_durability(&mut self, r: &mut BitReader<'_>, data: &(impl ExternalData + ?Sized), v: u32) -> Result<()> {
        let max_stat = data.stat_info(StatId::MAX_DURABILITY, v)?;
        let dur_stat = data.stat_info(StatId::DURABILITY, v)?;
        let max = (r.read_bits(max_stat.save_bits as u32)? as i32).wrapping_sub(max_stat.save_add);
        self.max_durability = Some(max);
        if max > 0 {
            self.durability = Some((r.read_bits(dur_stat.save_bits as u32)? as i32).wrapping_sub(dur_stat.save_add));
        }
        Ok(())
    }

    fn read_realm_data(&mut self, r: &mut BitReader<'_>, v: u32) -> Result<()> {
        if v <= 86 || !r.read_bool()? {
            return Ok(());
        }
        let count = match v {
            97.. => 4,
            94.. => 3,
            _ => 2,
        };
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(r.read_u32()?);
        }
        self.realm_data = Some(values);
        Ok(())
    }

    fn read_advanced_stash_stack_size(&mut self, r: &mut BitReader<'_>, v: u32) -> Result<()> {
        if v <= 99 {
            return Ok(());
        }
        let present = if v <= 101 { is_v100_stackable(self.code) } else { r.read_bool()? };
        if present {
            self.advanced_stash_stack_size = Some(r.read_u8()?);
        }
        Ok(())
    }

    fn read_chronicle(&mut self, r: &mut BitReader<'_>, v: u32) -> Result<()> {
        if v <= 99 || !self.flags.contains(ItemFlags::CHRONICLE) {
            return Ok(());
        }
        self.chronicle_id = Some(r.read_u16()?);
        let compact = self.flags.contains(ItemFlags::CHRONICLE_COMPACT);
        if !compact {
            self.chronicle_timestamp = Some(r.read_u32()?);
        }
        let count = if compact { 1 } else { (r.read_bits(4)? as usize).min(8) };
        if count == 0 {
            return Ok(());
        }
        let mut recipients = Vec::with_capacity(count);
        for _ in 0..count {
            // Serialized as two 32-bit halves, low first.
            recipients.push(r.read_u32()? as u64 | (r.read_u32()? as u64) << 32);
        }
        self.chronicle_recipients = Some(recipients);
        Ok(())
    }

    /// Writes the item (and its socketed items).
    pub fn write(&self, w: &mut BitWriter<'_>, data: &(impl ExternalData + ?Sized), save_version: u32) -> Result<()> {
        let v = save_version;
        if v <= 96 {
            w.write_u16(Self::MAGIC)?;
        }

        let info = data.item_info(self.code, v)?;
        let mut flags = (self.flags | ItemFlags::JUST_SAVED) - ItemFlags::INIT;
        if info.compact_save {
            flags |= ItemFlags::COMPACT_SAVE;
        }
        w.write_u32(flags.bits())?;

        if flags.contains(ItemFlags::COMPACT_SAVE) {
            self.write_compact(w, data, &info, v)?;
        } else {
            self.write_complete(w, data, &info, v)?;
        }

        w.align_to_byte();

        for socketed in self.socketed_items() {
            socketed.write(w, data, v)?;
        }
        Ok(())
    }

    fn write_compact(
        &self,
        w: &mut BitWriter<'_>,
        data: &(impl ExternalData + ?Sized),
        info: &ItemTypeInfo,
        v: u32,
    ) -> Result<()> {
        write_item_format(w, self.version, v)?;
        w.write_bits((self.position.mode.to_raw() as u32).min(7), 3)?;
        self.position.write(w)?;

        if self.flags.contains(ItemFlags::IS_EAR) {
            self.write_ear(w, v)?;
        } else {
            write_item_code(w, self.code, v)?;
            if info.is_gold {
                self.write_gold(w, v)?;
            }
        }

        if v > 92 && info.is_quest && info.quest_diff_check {
            let stat = data.stat_info(StatId::QUEST_ITEM_DIFFICULTY, v)?;
            let raw = self
                .quest_difficulty
                .map_or(0, |d| d.to_raw() as i32)
                .wrapping_shr(stat.val_shift as u32)
                .wrapping_add(stat.save_add);
            w.write_bits(raw as u32, stat.save_bits as u32)?;
        }

        self.write_realm_data(w, v)?;
        self.write_advanced_stash_stack_size(w, v)
    }

    fn write_complete(
        &self,
        w: &mut BitWriter<'_>,
        data: &(impl ExternalData + ?Sized),
        info: &ItemTypeInfo,
        v: u32,
    ) -> Result<()> {
        write_item_format(w, self.version, v)?;
        w.write_bits((self.position.mode.to_raw() as u32).min(7), 3)?;
        self.position.write(w)?;
        write_item_code(w, self.code, v)?;

        w.write_bits(self.socketed_items().count().min(7) as u32, 3)?;
        w.write_u32(self.seed)?;
        w.write_bits(self.level.clamp(1, 127) as u32, 7)?;
        w.write_bits(self.quality.to_raw() as u32, 4)?;

        w.write_bool(info.has_variable_gfx)?;
        if info.has_variable_gfx {
            w.write_bits(self.variable_gfx_id.unwrap_or(0).min(7) as u32, 3)?;
        }

        let auto_offset = data.auto_magic_offset(v);
        let auto_affix = self.auto_affix_id.unwrap_or(0) as i32;
        let adjusted = if auto_affix > auto_offset { auto_affix - auto_offset } else { auto_affix };
        w.write_bool(adjusted > 0)?;
        if adjusted > 0 {
            w.write_bits(adjusted as u32, 11)?;
        }

        if let Some(quality_data) = &self.quality_data {
            quality_data.write(w, data.magic_prefix_offset(v))?;
        }

        if self.flags.contains(ItemFlags::RUNEWORD) {
            w.write_u16(self.runeword_id.unwrap_or(0))?;
        }

        if self.flags.contains(ItemFlags::IS_EAR) {
            self.write_ear(w, v)?;
        } else if self.flags.contains(ItemFlags::PERSONALIZED) {
            write_string(w, &self.personalized_name, v)?;
        }

        self.write_realm_data(w, v)?;

        if info.is_armor {
            let ac = data.stat_info(StatId::ARMOR_CLASS, v)?;
            let defense = clamp_semantic(self.defense.unwrap_or(0), ac.max_semantic_value())?;
            w.write_bits(defense.wrapping_add(ac.save_add) as u32, ac.save_bits as u32)?;
            self.write_durability(w, data, v)?;
        } else if info.is_weapon {
            self.write_durability(w, data, v)?;
        } else if info.is_gold {
            self.write_gold(w, v)?;
        }

        if v > 104 {
            w.write_bool(self.quantity.is_some())?;
            if let Some(quantity) = self.quantity {
                w.write_bits(quantity.clamp(0, 511) as u32, 9)?;
            }
        } else if info.is_stackable {
            let bits = data.quantity_bits(v);
            let max = (1i32 << bits) - 1;
            w.write_bits(self.quantity.unwrap_or(0).clamp(0, max) as u32, bits)?;
        }

        if self.flags.contains(ItemFlags::SOCKETED) {
            let stat = data.stat_info(StatId::NUM_SOCKETS, v)?;
            let max = 1i32.wrapping_shl(stat.save_bits as u32).wrapping_sub(1);
            let count = clamp_semantic(self.sockets.len().min(i32::MAX as usize) as i32, max)?;
            w.write_bits(count as u32, stat.save_bits as u32)?;
        }

        if self.quality == ItemQuality::Set {
            w.write_bits(self.set_item_mask.unwrap_or(0) as u32, 5)?;
        }

        write_stat_list(w, &self.stats, data, v)?;

        if let (ItemQuality::Set, Some(mask)) = (self.quality, self.set_item_mask) {
            let mut bonus = self.set_bonus_stats.iter();
            for i in 0..5 {
                if mask & (1 << i) != 0 {
                    write_stat_list(w, bonus.next().map_or(&[][..], Vec::as_slice), data, v)?;
                }
            }
        }

        if self.flags.contains(ItemFlags::RUNEWORD) {
            write_stat_list(w, self.runeword_stats.as_deref().unwrap_or(&[]), data, v)?;
        }

        self.write_chronicle(w, v)?;
        self.write_advanced_stash_stack_size(w, v)
    }

    fn write_ear(&self, w: &mut BitWriter<'_>, v: u32) -> Result<()> {
        w.write_bits(self.ear_file_index.min(7) as u32, 3)?;
        w.write_bits(self.ear_level.min(127) as u32, 7)?;
        write_string(w, &self.ear_name, v)
    }

    fn write_gold(&self, w: &mut BitWriter<'_>, v: u32) -> Result<()> {
        let big = self.gold_amount >= 4096;
        w.write_bool(big)?;
        if big {
            w.write_u32(self.gold_amount)?;
        } else {
            w.write_bits(self.gold_amount, 12)?;
        }
        if v > 96 {
            w.write_bool(self.player_gold)?;
        }
        Ok(())
    }

    fn write_durability(&self, w: &mut BitWriter<'_>, data: &(impl ExternalData + ?Sized), v: u32) -> Result<()> {
        let max_stat = data.stat_info(StatId::MAX_DURABILITY, v)?;
        let dur_stat = data.stat_info(StatId::DURABILITY, v)?;
        let max = self.max_durability.unwrap_or(0);
        let max_clamped = clamp_semantic(max, max_stat.max_semantic_value())?;
        w.write_bits(max_clamped.wrapping_add(max_stat.save_add) as u32, max_stat.save_bits as u32)?;
        if max > 0 {
            let dur = clamp_semantic(self.durability.unwrap_or(0), dur_stat.max_semantic_value())?;
            w.write_bits(dur.wrapping_add(dur_stat.save_add) as u32, dur_stat.save_bits as u32)?;
        }
        Ok(())
    }

    fn write_realm_data(&self, w: &mut BitWriter<'_>, v: u32) -> Result<()> {
        if v <= 86 {
            return Ok(());
        }
        let realm = self.realm_data.as_deref().filter(|d| d.iter().any(|&x| x != 0));
        w.write_bool(realm.is_some())?;
        let Some(realm) = realm else { return Ok(()) };
        let at = |i: usize| realm.get(i).copied().unwrap_or(0);
        if v > 96 {
            if realm.len() < 4 {
                // Read from the old format: the game loads the two old values into [2] and [3].
                for value in [0, 0, at(0), at(1)] {
                    w.write_u32(value)?;
                }
            } else {
                for value in &realm[..4] {
                    w.write_u32(*value)?;
                }
            }
        } else {
            // Two values (plus one discarded value for v94-96).
            let offset = if realm.len() >= 4 { 2 } else { 0 };
            w.write_u32(at(offset))?;
            w.write_u32(at(offset + 1))?;
            if v > 93 {
                w.write_u32(if offset == 0 { at(2) } else { 0 })?;
            }
        }
        Ok(())
    }

    fn write_advanced_stash_stack_size(&self, w: &mut BitWriter<'_>, v: u32) -> Result<()> {
        if v <= 99 {
            return Ok(());
        }
        if v <= 101 {
            if is_v100_stackable(self.code) {
                w.write_u8(self.advanced_stash_stack_size.unwrap_or(0))?;
            }
            return Ok(());
        }
        w.write_bool(self.advanced_stash_stack_size.is_some())?;
        if let Some(size) = self.advanced_stash_stack_size {
            w.write_u8(size)?;
        }
        Ok(())
    }

    fn write_chronicle(&self, w: &mut BitWriter<'_>, v: u32) -> Result<()> {
        if v <= 99 || !self.flags.contains(ItemFlags::CHRONICLE) {
            return Ok(());
        }
        w.write_u16(self.chronicle_id.unwrap_or(0))?;
        let compact = self.flags.contains(ItemFlags::CHRONICLE_COMPACT);
        if !compact {
            w.write_u32(self.chronicle_timestamp.unwrap_or(0))?;
        }
        let recipients = self.chronicle_recipients.as_deref().unwrap_or(&[]);
        let count = if compact {
            1
        } else {
            let count = recipients.len().min(8);
            w.write_bits(count as u32, 4)?;
            count
        };
        for &id in recipients.iter().take(count) {
            w.write_u32(id as u32)?;
            w.write_u32((id >> 32) as u32)?;
        }
        Ok(())
    }
}

/// Clamps a semantic value to `[0, max]` (fails if `max` is negative).
fn clamp_semantic(value: i32, max: i32) -> Result<i32> {
    if max < 0 {
        return Err(Error::invalid_data(format!("invalid stat range: max {max} is negative")));
    }
    Ok(value.clamp(0, max))
}

fn read_quality_data(
    r: &mut BitReader<'_>,
    quality: ItemQuality,
    info: &ItemTypeInfo,
    offset: i32,
) -> Result<Option<QualityData>> {
    Ok(Some(match quality {
        ItemQuality::Inferior => QualityData::read_inferior(r)?,
        ItemQuality::Superior => QualityData::read_superior(r)?,
        ItemQuality::Magic => QualityData::read_magic(r, offset)?,
        ItemQuality::Set | ItemQuality::Unique => QualityData::read_set_unique(r)?,
        ItemQuality::Rare | ItemQuality::Craft => QualityData::read_rare_craft(r, offset)?,
        ItemQuality::Tempered => QualityData::read_tempered(r)?,
        ItemQuality::Normal if info.is_charm => QualityData::read_charm(r, offset)?,
        ItemQuality::Normal if info.is_body_part && !info.is_player_body_part => QualityData::read_body_part(r)?,
        ItemQuality::Normal if info.is_scroll_or_book => QualityData::read_scroll_book(r)?,
        _ => return Ok(None),
    }))
}

/// Reads an item stat list (with paired stats) up to the terminator.
pub(crate) fn read_stat_list(r: &mut BitReader<'_>, data: &(impl ExternalData + ?Sized), v: u32) -> Result<Vec<Stat>> {
    let mut stats = Vec::new();
    loop {
        let id = StatId(r.read_bits(9)? as u16);
        if id == StatId::TERMINATOR {
            break;
        }
        let info = data.stat_info(id, v)?;
        let layer = if info.save_param_bits > 0 { r.read_bits(info.save_param_bits as u32)? as i32 } else { 0 };
        let raw = (r.read_bits(info.save_bits as u32)? as i32).wrapping_sub(info.save_add);
        stats.push(Stat { id, layer, value: (raw as i64).wrapping_shl(info.val_shift as u32) });

        for &paired in id.paired_stats() {
            let pinfo = data.stat_info(paired, v)?;
            let raw = (r.read_bits(pinfo.save_bits as u32)? as i32).wrapping_sub(pinfo.save_add);
            stats.push(Stat { id: paired, layer: 0, value: (raw as i64).wrapping_shl(pinfo.val_shift as u32) });
        }

        if id == StatId::POISON_MIN_DAMAGE {
            stats.push(Stat { id: StatId::POISON_COUNT, layer: 0, value: 1 });
        }
    }
    Ok(stats)
}

/// Writes an item stat list (with paired stats) and the terminator.
pub(crate) fn write_stat_list(
    w: &mut BitWriter<'_>,
    stats: &[Stat],
    data: &(impl ExternalData + ?Sized),
    v: u32,
) -> Result<()> {
    // Stats already written as part of a paired group.
    let mut written: Vec<StatId> = Vec::new();
    for stat in stats {
        if stat.id == StatId::POISON_COUNT || written.contains(&stat.id) {
            continue;
        }
        let info = data.stat_info(stat.id, v)?;
        if info.save_bits == 0 {
            continue;
        }
        let shifted = stat.value.wrapping_shr(info.val_shift as u32);
        if shifted == 0 {
            continue;
        }

        w.write_bits(stat.id.0 as u32, 9)?;
        if info.save_param_bits > 0 {
            w.write_bits(stat.layer as u32, info.save_param_bits as u32)?;
        }
        w.write_bits(shifted.wrapping_add(info.save_add as i64) as u32, info.save_bits as u32)?;

        for &paired in stat.id.paired_stats() {
            // The last stat with the paired ID wins, matching a dictionary built in list order.
            let value = stats.iter().rev().find(|s| s.id == paired).map_or(0, |s| s.value);
            let pinfo = data.stat_info(paired, v)?;
            written.push(paired);
            let pshifted = value.wrapping_shr(pinfo.val_shift as u32);
            w.write_bits(pshifted.wrapping_add(pinfo.save_add as i64) as u32, pinfo.save_bits as u32)?;
        }
    }
    w.write_bits(StatId::TERMINATOR.0 as u32, 9)
}

fn read_item_format(r: &mut BitReader<'_>, v: u32) -> Result<u16> {
    if v > 96 {
        let high = r.read_bool()?;
        let value = r.read_bits(2)? as u16;
        Ok(if high { value + 99 } else { value })
    } else {
        Ok(r.read_bits(10)? as u16)
    }
}

fn write_item_format(w: &mut BitWriter<'_>, version: u16, v: u32) -> Result<()> {
    let version = version as u32;
    if v > 96 {
        if version >= 100 {
            w.write_bool(true)?;
            w.write_bits((version - 99).min(3), 2)
        } else {
            w.write_bool(false)?;
            w.write_bits(version.min(3), 2)
        }
    } else {
        w.write_bits(version.min(1023), 10)
    }
}

fn read_item_code(r: &mut BitReader<'_>, v: u32) -> Result<ItemCode> {
    Ok(ItemCode(if v > 96 { huffman::decode(r)? } else { r.read_u32()? }))
}

fn write_item_code(w: &mut BitWriter<'_>, code: ItemCode, v: u32) -> Result<()> {
    if v > 96 {
        huffman::encode(w, code.0)
    } else {
        w.write_u32(code.0)
    }
}

fn read_string(r: &mut BitReader<'_>, v: u32) -> Result<String> {
    if v > 97 {
        r.read_8bit_string()
    } else {
        r.read_7bit_string()
    }
}

fn write_string(w: &mut BitWriter<'_>, s: &str, v: u32) -> Result<()> {
    if v > 97 {
        w.write_8bit_string(s)
    } else {
        w.write_7bit_string(s)
    }
}
