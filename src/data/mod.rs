//! Game data required to parse items and stats.

mod txt;

pub use txt::TxtData;

use crate::error::Result;
use crate::{ItemCode, StatId};

/// Information about a stat from ItemStatCost.txt.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct StatInfo {
    /// Bits for the player stat value (`CSvBits`).
    pub csv_bits: u8,
    /// Bits for the player stat parameter (`CSvParam`).
    pub csv_param: u8,
    /// Whether the player stat value is signed (`CSvSigned`).
    pub csv_signed: bool,
    /// Bits for the item stat value (`Save Bits`).
    pub save_bits: i32,
    /// Bits for the item stat parameter (`Save Param Bits`).
    pub save_param_bits: i32,
    /// Offset added when saving (`Save Add`).
    pub save_add: i32,
    /// Fixed-point shift (`ValShift`). Read: `(raw - SaveAdd) << ValShift`.
    pub val_shift: u8,
}

impl StatInfo {
    /// Maximum semantic value that can be stored: `(1 << SaveBits) - 1 - SaveAdd`.
    pub fn max_semantic_value(&self) -> i32 {
        1i32.wrapping_shl(self.save_bits as u32).wrapping_sub(1).wrapping_sub(self.save_add)
    }
}

/// Information about an item type from the item tables and ItemTypes.txt.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ItemTypeInfo {
    /// Item uses the compact save format.
    pub compact_save: bool,
    /// Item has variable inventory graphics.
    pub has_variable_gfx: bool,
    /// Item is an armor type.
    pub is_armor: bool,
    /// Item is a weapon type.
    pub is_weapon: bool,
    /// Item is gold.
    pub is_gold: bool,
    /// Item is stackable.
    pub is_stackable: bool,
    /// Item is a charm.
    pub is_charm: bool,
    /// Item is a monster body part.
    pub is_body_part: bool,
    /// Item is a player body part.
    pub is_player_body_part: bool,
    /// Item is a scroll or book.
    pub is_scroll_or_book: bool,
    /// Item is a quest item.
    pub is_quest: bool,
    /// Quest item has a difficulty check.
    pub quest_diff_check: bool,
}

/// Provides the game data needed for parsing stats and items.
///
/// [`TxtData`] implements this using the original game .txt files; the
/// embedded default is available through [`TxtData::embedded`].
pub trait ExternalData {
    /// Stat information for a stat ID.
    fn stat_info(&self, stat: StatId, save_version: u32) -> Result<StatInfo>;

    /// Item type information for an item code. Fails for unknown codes.
    fn item_info(&self, code: ItemCode, save_version: u32) -> Result<ItemTypeInfo>;

    /// Bit width of item quantities (9 bits for version > 80, else 8).
    fn quantity_bits(&self, save_version: u32) -> u32 {
        if save_version > 80 {
            9
        } else {
            8
        }
    }

    /// Magic prefix ID offset applied to magic prefixes.
    fn magic_prefix_offset(&self, save_version: u32) -> i32 {
        match save_version {
            0x48 | 0x49 | 0x50..=0x52 => -661,
            0x53 | 0x54 => -674,
            0x55..=0x57 => -675,
            0x58 => -729,
            _ => 0,
        }
    }

    /// Auto magic affix ID offset.
    fn auto_magic_offset(&self, save_version: u32) -> i32 {
        match save_version {
            0x48 | 0x49 | 0x50..=0x52 => -1262,
            0x53 | 0x54 => -1265,
            0x55..=0x57 => -1266,
            0x58 => -1440,
            _ => 0,
        }
    }
}

impl<T: ExternalData + ?Sized> ExternalData for &T {
    fn stat_info(&self, stat: StatId, save_version: u32) -> Result<StatInfo> {
        (**self).stat_info(stat, save_version)
    }
    fn item_info(&self, code: ItemCode, save_version: u32) -> Result<ItemTypeInfo> {
        (**self).item_info(code, save_version)
    }
    fn quantity_bits(&self, save_version: u32) -> u32 {
        (**self).quantity_bits(save_version)
    }
    fn magic_prefix_offset(&self, save_version: u32) -> i32 {
        (**self).magic_prefix_offset(save_version)
    }
    fn auto_magic_offset(&self, save_version: u32) -> i32 {
        (**self).auto_magic_offset(save_version)
    }
}

#[cfg(feature = "embedded-data")]
/// The default external data (embedded game tables for versions 96, 97, 99 and 105).
pub fn default_data() -> &'static TxtData {
    TxtData::embedded()
}
