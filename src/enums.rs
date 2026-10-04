//! Enumerations and flag sets used in save files.
//!
//! Enumerations read from bit fields are "open": values that have no named
//! variant are preserved in an `Unknown` variant so that they round-trip unchanged.

#![allow(missing_docs)] // variant and flag names are self-describing

use bitflags::bitflags;

macro_rules! open_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident : $repr:ty {
            $( $(#[$vmeta:meta])* $variant:ident = $value:literal ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )*
            /// A value with no named variant.
            Unknown($repr),
        }

        impl $name {
            /// Converts a raw value, preserving unknown values.
            pub const fn from_raw(value: $repr) -> Self {
                match value {
                    $( $value => Self::$variant, )*
                    other => Self::Unknown(other),
                }
            }

            /// Returns the raw value.
            pub const fn to_raw(self) -> $repr {
                match self {
                    $( Self::$variant => $value, )*
                    Self::Unknown(value) => value,
                }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::from_raw(0)
            }
        }

        impl From<$repr> for $name {
            fn from(value: $repr) -> Self {
                Self::from_raw(value)
            }
        }

        impl From<$name> for $repr {
            fn from(value: $name) -> Self {
                value.to_raw()
            }
        }

        impl PartialOrd for $name {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for $name {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.to_raw().cmp(&other.to_raw())
            }
        }
    };
}

open_enum! {
    /// Body equipment locations.
    pub enum BodyLocation: u8 {
        None = 0,
        Head = 1,
        Neck = 2,
        Torso = 3,
        RightArm = 4,
        LeftArm = 5,
        RightRing = 6,
        LeftRing = 7,
        Belt = 8,
        Feet = 9,
        Gloves = 10,
        RightHand = 11,
        LeftHand = 12,
    }
}

open_enum! {
    /// Character classes.
    pub enum CharacterClass: u8 {
        Amazon = 0,
        Sorceress = 1,
        Necromancer = 2,
        Paladin = 3,
        Barbarian = 4,
        Druid = 5,
        Assassin = 6,
        Warlock = 7,
    }
}

open_enum! {
    /// Game difficulty levels.
    pub enum Difficulty: u8 {
        Normal = 0,
        Nightmare = 1,
        Hell = 2,
    }
}

open_enum! {
    /// Game version stored in the preview data of v104+ saves.
    pub enum GameVersion: u8 {
        None = 0,
        Classic = 1,
        Expansion = 2,
        ReignOfTheWarlock = 3,
    }
}

open_enum! {
    /// Item animation/storage mode.
    pub enum ItemMode: u8 {
        /// Stored in inventory, stash or cube.
        Stored = 0,
        /// Equipped on the character.
        Equipped = 1,
        /// In the belt.
        InBelt = 2,
        /// On the ground.
        OnGround = 3,
        /// On the cursor.
        OnCursor = 4,
        /// Dropping or being thrown.
        Dropping = 5,
        /// Socketed in another item.
        Socketed = 6,
    }
}

open_enum! {
    /// Item quality levels.
    pub enum ItemQuality: u8 {
        Inferior = 1,
        Normal = 2,
        Superior = 3,
        Magic = 4,
        Set = 5,
        Rare = 6,
        Unique = 7,
        Craft = 8,
        Tempered = 9,
    }
}

open_enum! {
    /// Shared stash tab type (stash format >= 2).
    pub enum StashTabType: u8 {
        /// Normal tab containing items.
        Normal = 0,
        /// Advanced stash tab with stackable item support.
        AdvancedStash = 1,
        /// Chronicle tab tracking found set/unique/runeword items.
        Chronicle = 2,
    }
}

open_enum! {
    /// Storage page of a stored item.
    ///
    /// The file stores the page + 1; a raw value of 0 is read as [`StorePage::Null`].
    pub enum StorePage: u8 {
        Inventory = 0,
        Equip = 1,
        Trade = 2,
        Cube = 3,
        Stash = 4,
        Belt = 5,
        Null = 255,
    }
}

open_enum! {
    /// Bound demon type.
    pub enum DemonType: u16 {
        /// Standard monster; class ID is a MonStats.txt row.
        Standard = 1,
        /// Super unique; class ID is a SuperUniques.txt row.
        SuperUnique = 2,
    }
}

/// Side of the skill bar a skill is assigned to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Side {
    /// Right side.
    #[default]
    Right = 0,
    /// Left side.
    Left = 1,
}

/// Appearance composit indexes (into the appearance graphics/tints arrays).
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Composit {
    Head,
    Torso,
    Legs,
    RightArm,
    LeftArm,
    RightHand,
    LeftHand,
    Shield,
    Special1,
    Special2,
    Special3,
    Special4,
    Special5,
    Special6,
    Special7,
    Special8,
}

bitflags! {
    /// Character state flags stored in the save header.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[cfg_attr(feature = "serde", serde(transparent))]
    pub struct CharacterFlags: u32 {
        const INIT = 0x1;
        const REALM_CHAR = 0x2;
        const HARDCORE = 0x4;
        const DEAD = 0x8;
        const SAVE_PROCESS = 0x10;
        const EXPANSION = 0x20;
        const LADDER = 0x40;
        const NEEDS_RENAMING = 0x80;
        const WEAPON_SWITCH = 0x2000;
        const _ = !0;
    }
}

impl CharacterFlags {
    const PROGRESSION_MASK: u32 = 0x1F00;

    /// Progression value (each act completion increments it).
    pub fn progression(self) -> u32 {
        (self.bits() & Self::PROGRESSION_MASK) >> 8
    }

    /// Sets the progression value.
    pub fn set_progression(&mut self, progression: u32) {
        *self = Self::from_bits_retain(
            (self.bits() & !Self::PROGRESSION_MASK) | ((progression << 8) & Self::PROGRESSION_MASK),
        );
    }

    /// Maximum difficulty the character can access based on progression.
    pub fn max_allowed_difficulty(self) -> Difficulty {
        let acts_per_difficulty = if self.contains(Self::EXPANSION) { 5 } else { 4 };
        Difficulty::from_raw((self.progression() / acts_per_difficulty).min(2) as u8)
    }
}

bitflags! {
    /// Item state flags (first 32 bits of an item record).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[cfg_attr(feature = "serde", serde(transparent))]
    pub struct ItemFlags: u32 {
        const NEW_ITEM = 0x00000001;
        const TARGET = 0x00000002;
        const TARGETING = 0x00000004;
        const DELETED = 0x00000008;
        const IDENTIFIED = 0x00000010;
        const HAS_QUANTITY = 0x00000020;
        const SWITCH_IN = 0x00000040;
        const SWITCH_OUT = 0x00000080;
        const BROKEN = 0x00000100;
        const REPAIRED = 0x00000200;
        const SOCKETED = 0x00000800;
        const NO_SELL = 0x00001000;
        const IN_STORE = 0x00002000;
        const NO_EQUIP = 0x00004000;
        const NAMED = 0x00008000;
        const IS_EAR = 0x00010000;
        const STARTER_ITEM = 0x00020000;
        const INIT = 0x00080000;
        const COMPACT_SAVE = 0x00200000;
        const ETHEREAL = 0x00400000;
        const JUST_SAVED = 0x00800000;
        const PERSONALIZED = 0x01000000;
        const LOW_QUALITY = 0x02000000;
        const RUNEWORD = 0x04000000;
        const DUPLICATED = 0x08000000;
        const CHRONICLE = 0x10000000;
        const CHRONICLE_COMPACT = 0x20000000;
        const _ = !0;
    }
}

bitflags! {
    /// Quest progress flags stored per quest per difficulty.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[cfg_attr(feature = "serde", serde(transparent))]
    pub struct QuestFlags: u16 {
        const REWARD_GRANTED = 0x1;
        const REWARD_PENDING = 0x2;
        const STARTED = 0x4;
        const LEFT_TOWN = 0x8;
        const ENTER_AREA = 0x10;
        const CUSTOM1 = 0x20;
        const CUSTOM2 = 0x40;
        const CUSTOM3 = 0x80;
        const CUSTOM4 = 0x100;
        const CUSTOM5 = 0x200;
        const CUSTOM6 = 0x400;
        const CUSTOM7 = 0x800;
        const QUEST_LOG = 0x1000;
        const PRIMARY_GOAL_ACHIEVED = 0x2000;
        const COMPLETED_NOW = 0x4000;
        const COMPLETED_BEFORE = 0x8000;
    }
}

bitflags! {
    /// Act I waypoints.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[cfg_attr(feature = "serde", serde(transparent))]
    pub struct ActIWaypoints: u16 {
        const ROGUE_ENCAMPMENT = 0x001;
        const COLD_PLAINS = 0x002;
        const STONY_FIELD = 0x004;
        const DARK_WOOD = 0x008;
        const BLACK_MARSH = 0x010;
        const OUTER_CLOISTER = 0x020;
        const JAIL_LEVEL1 = 0x040;
        const INNER_CLOISTER = 0x080;
        const CATACOMBS_LEVEL2 = 0x100;
        const _ = !0;
    }
}

bitflags! {
    /// Act II waypoints.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[cfg_attr(feature = "serde", serde(transparent))]
    pub struct ActIIWaypoints: u16 {
        const LUT_GHOLEIN = 0x001;
        const SEWERS_LEVEL2 = 0x002;
        const DRY_HILLS = 0x004;
        const HALLS_OF_THE_DEAD_LEVEL2 = 0x008;
        const FAR_OASIS = 0x010;
        const LOST_CITY = 0x020;
        const PALACE_CELLAR_LEVEL1 = 0x040;
        const ARCANE_SANCTUARY = 0x080;
        const CANYON_OF_THE_MAGI = 0x100;
        const _ = !0;
    }
}

bitflags! {
    /// Act III waypoints.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[cfg_attr(feature = "serde", serde(transparent))]
    pub struct ActIIIWaypoints: u16 {
        const KURAST_DOCKS = 0x001;
        const SPIDER_FOREST = 0x002;
        const GREAT_MARSH = 0x004;
        const FLAYER_JUNGLE = 0x008;
        const LOWER_KURAST = 0x010;
        const KURAST_BAZAAR = 0x020;
        const UPPER_KURAST = 0x040;
        const TRAVINCAL = 0x080;
        const DURANCE_OF_HATE_LEVEL2 = 0x100;
        const _ = !0;
    }
}

bitflags! {
    /// Act IV waypoints.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[cfg_attr(feature = "serde", serde(transparent))]
    pub struct ActIVWaypoints: u8 {
        const THE_PANDEMONIUM_FORTRESS = 0x1;
        const CITY_OF_THE_DAMNED = 0x2;
        const RIVER_OF_FLAME = 0x4;
        const _ = !0;
    }
}

bitflags! {
    /// Act V waypoints.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[cfg_attr(feature = "serde", serde(transparent))]
    pub struct ActVWaypoints: u16 {
        const HARROGATH = 0x001;
        const FRIGID_HIGHLANDS = 0x002;
        const ARREAT_PLATEAU = 0x004;
        const CRYSTALLINE_PASSAGE = 0x008;
        const HALLS_OF_PAIN = 0x010;
        const GLACIAL_TRAIL = 0x020;
        const FROZEN_TUNDRA = 0x040;
        const THE_ANCIENTS_WAY = 0x080;
        const WORLDSTONE_KEEP_LEVEL2 = 0x100;
        const _ = !0;
    }
}

/// A 4-character item code packed little-endian into a `u32` (e.g. `"cap "`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct ItemCode(pub u32);

impl ItemCode {
    /// The ear item code (`"ear "`).
    pub const EAR: ItemCode = ItemCode(0x20726165);
    /// The gold item code (`"gld "`).
    pub const GOLD: ItemCode = ItemCode(0x20646c67);

    /// Creates a code from a string, padding with spaces (extra characters are ignored).
    pub const fn new(code: &str) -> ItemCode {
        let b = code.as_bytes();
        let mut out = [b' '; 4];
        let mut i = 0;
        while i < 4 && i < b.len() {
            out[i] = b[i];
            i += 1;
        }
        ItemCode(u32::from_le_bytes(out))
    }

    /// The raw code bytes.
    pub const fn bytes(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }

    /// The code as a string with trailing spaces/nulls trimmed.
    pub fn as_string(self) -> String {
        let bytes = self.bytes();
        let mut len = 4;
        while len > 0 && (bytes[len - 1] == b' ' || bytes[len - 1] == 0) {
            len -= 1;
        }
        bytes[..len].iter().map(|&b| b as char).collect()
    }
}

impl std::fmt::Display for ItemCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.as_string())
    }
}

impl std::fmt::Debug for ItemCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ItemCode({:?})", self.as_string())
    }
}

impl From<&str> for ItemCode {
    fn from(code: &str) -> Self {
        ItemCode::new(code)
    }
}

impl From<u32> for ItemCode {
    fn from(code: u32) -> Self {
        ItemCode(code)
    }
}

impl PartialEq<&str> for ItemCode {
    fn eq(&self, other: &&str) -> bool {
        *self == ItemCode::new(other)
    }
}
