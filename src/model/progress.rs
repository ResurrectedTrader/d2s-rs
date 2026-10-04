//! Fixed-size progression sections: quests, waypoints and NPC introductions.

use crate::bits::{BitReader, BitWriter};
use crate::enums::{
    ActIIIWaypoints, ActIIWaypoints, ActIVWaypoints, ActIWaypoints, ActVWaypoints, Difficulty, QuestFlags,
};
use crate::error::{Error, Result};

macro_rules! quest_act {
    ($(#[$meta:meta])* $name:ident { $($field:ident),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[allow(missing_docs)]
        pub struct $name {
            $(pub $field: QuestFlags,)*
        }

        impl $name {
            /// Reads the act's quest slots.
            pub fn read(r: &mut BitReader<'_>) -> Result<Self> {
                Ok($name { $($field: QuestFlags::from_bits_retain(r.read_u16()?),)* })
            }

            /// Writes the act's quest slots.
            pub fn write(&self, w: &mut BitWriter<'_>) -> Result<()> {
                $(w.write_u16(self.$field.bits())?;)*
                Ok(())
            }

            /// The quest slots in file order.
            pub fn slots(&self) -> Vec<QuestFlags> {
                vec![$(self.$field),*]
            }

            /// Mutable references to the quest slots in file order.
            pub fn slots_mut(&mut self) -> Vec<&mut QuestFlags> {
                vec![$(&mut self.$field),*]
            }
        }
    };
}

quest_act! {
    /// Act I quests (8 slots).
    ActIQuests {
        introduction, den_of_evil, sisters_burial_grounds, tools_of_the_trade,
        the_search_for_cain, the_forgotten_tower, sisters_to_the_slaughter, completion,
    }
}

quest_act! {
    /// Act II quests (8 slots).
    ActIIQuests {
        introduction, radaments_lair, the_horadric_staff, tainted_sun,
        arcane_sanctuary, the_summoner, the_seven_tombs, completion,
    }
}

quest_act! {
    /// Act III quests (8 slots).
    ActIIIQuests {
        introduction, lam_esens_tome, khalims_will, blade_of_the_old_religion,
        the_golden_bird, the_blackened_temple, the_guardian, completion,
    }
}

quest_act! {
    /// Act IV quests (8 slots). Only 4 actual quests; the last 3 slots hold gossip
    /// state for NPCs from other acts (quest record slots 29-31).
    ///
    /// - `flavie_gossip`: Act I, Flavie's gossip about the Den of Evil (A1Q7). Typically 0.
    /// - `guard_gossip1`: Act II guard gossip (A2Q7). Bit 0 set on talk, bit 13 on specific messages.
    /// - `guard_gossip2`: Act II guard gossip (A2Q8). Bit 0 set on talk.
    ActIVQuests {
        introduction, the_fallen_angel, terrors_end, hellforge,
        completion, flavie_gossip, guard_gossip1, guard_gossip2,
    }
}

quest_act! {
    /// Act V quests (16 slots, quest record slots 32-47). The game only processes
    /// slots up to 41; `unused42`..`unused47` are never used.
    ///
    /// - `dark_wanderer`: Act III Dark Wanderer event (A3Q7). Bit 0 set once seen.
    /// - `hadriel_gossip`: Act IV Hadriel's gossip (A4Q4). Typically 0.
    /// - `akara_respec`: Akara's stat/skill reset. [`QuestFlags::REWARD_GRANTED`] = used,
    ///   [`QuestFlags::REWARD_PENDING`] = available.
    ActVQuests {
        dark_wanderer, hadriel_gossip, unused34, siege_on_harrogath,
        rescue_on_mount_arreat, prison_of_ice, betrayal_of_harrogath, rite_of_passage,
        eve_of_destruction, akara_respec, unused42, unused43,
        unused44, unused45, unused46, unused47,
    }
}

/// Quest data for one difficulty (96 bytes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub struct QuestsDifficulty {
    pub act1: ActIQuests,
    pub act2: ActIIQuests,
    pub act3: ActIIIQuests,
    pub act4: ActIVQuests,
    pub act5: ActVQuests,
}

impl QuestsDifficulty {
    /// Reads quest data for one difficulty.
    pub fn read(r: &mut BitReader<'_>) -> Result<Self> {
        Ok(QuestsDifficulty {
            act1: ActIQuests::read(r)?,
            act2: ActIIQuests::read(r)?,
            act3: ActIIIQuests::read(r)?,
            act4: ActIVQuests::read(r)?,
            act5: ActVQuests::read(r)?,
        })
    }

    /// Writes quest data for one difficulty.
    pub fn write(&self, w: &mut BitWriter<'_>) -> Result<()> {
        self.act1.write(w)?;
        self.act2.write(w)?;
        self.act3.write(w)?;
        self.act4.write(w)?;
        self.act5.write(w)
    }
}

/// Quest section (298 bytes, magic "Woo!").
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QuestSection {
    /// Section version (6).
    pub version: u32,
    /// Section size field (298).
    pub section_size: u16,
    /// Normal difficulty.
    pub normal: QuestsDifficulty,
    /// Nightmare difficulty.
    pub nightmare: QuestsDifficulty,
    /// Hell difficulty.
    pub hell: QuestsDifficulty,
}

impl Default for QuestSection {
    fn default() -> Self {
        QuestSection {
            version: 6,
            section_size: Self::SIZE as u16,
            normal: Default::default(),
            nightmare: Default::default(),
            hell: Default::default(),
        }
    }
}

impl QuestSection {
    /// Section magic "Woo!".
    pub const MAGIC: u32 = 0x216F6F57;
    /// Section size in bytes.
    pub const SIZE: usize = 298;

    /// Quest data for a difficulty.
    pub fn get(&self, difficulty: Difficulty) -> Option<&QuestsDifficulty> {
        match difficulty {
            Difficulty::Normal => Some(&self.normal),
            Difficulty::Nightmare => Some(&self.nightmare),
            Difficulty::Hell => Some(&self.hell),
            Difficulty::Unknown(_) => None,
        }
    }

    /// Mutable quest data for a difficulty.
    pub fn get_mut(&mut self, difficulty: Difficulty) -> Option<&mut QuestsDifficulty> {
        match difficulty {
            Difficulty::Normal => Some(&mut self.normal),
            Difficulty::Nightmare => Some(&mut self.nightmare),
            Difficulty::Hell => Some(&mut self.hell),
            Difficulty::Unknown(_) => None,
        }
    }

    /// Reads the quest section.
    pub fn read(r: &mut BitReader<'_>) -> Result<Self> {
        let magic = r.read_u32()?;
        if magic != Self::MAGIC {
            return Err(Error::InvalidMagic { section: "quest section", found: magic });
        }
        Ok(QuestSection {
            version: r.read_u32()?,
            section_size: r.read_u16()?,
            normal: QuestsDifficulty::read(r)?,
            nightmare: QuestsDifficulty::read(r)?,
            hell: QuestsDifficulty::read(r)?,
        })
    }

    /// Writes the quest section.
    pub fn write(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.write_u32(Self::MAGIC)?;
        w.write_u32(self.version)?;
        w.write_u16(self.section_size)?;
        self.normal.write(w)?;
        self.nightmare.write(w)?;
        self.hell.write(w)
    }
}

/// Waypoint section (80 bytes, magic "WS").
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WaypointSection {
    /// Section version (1).
    pub version: u32,
    /// Section size field (80).
    pub section_size: u16,
    /// Normal difficulty.
    pub normal: WaypointDifficulty,
    /// Nightmare difficulty.
    pub nightmare: WaypointDifficulty,
    /// Hell difficulty.
    pub hell: WaypointDifficulty,
}

impl Default for WaypointSection {
    fn default() -> Self {
        WaypointSection {
            version: 1,
            section_size: Self::SIZE as u16,
            normal: Default::default(),
            nightmare: Default::default(),
            hell: Default::default(),
        }
    }
}

impl WaypointSection {
    /// Section magic "WS".
    pub const MAGIC: u16 = 0x5357;
    /// Section size in bytes.
    pub const SIZE: usize = 80;

    /// Waypoints for a difficulty.
    pub fn get(&self, difficulty: Difficulty) -> Option<&WaypointDifficulty> {
        match difficulty {
            Difficulty::Normal => Some(&self.normal),
            Difficulty::Nightmare => Some(&self.nightmare),
            Difficulty::Hell => Some(&self.hell),
            Difficulty::Unknown(_) => None,
        }
    }

    /// Mutable waypoints for a difficulty.
    pub fn get_mut(&mut self, difficulty: Difficulty) -> Option<&mut WaypointDifficulty> {
        match difficulty {
            Difficulty::Normal => Some(&mut self.normal),
            Difficulty::Nightmare => Some(&mut self.nightmare),
            Difficulty::Hell => Some(&mut self.hell),
            Difficulty::Unknown(_) => None,
        }
    }

    /// Unlocks every waypoint in every difficulty.
    pub fn unlock_all(&mut self) {
        self.normal.unlock_all();
        self.nightmare.unlock_all();
        self.hell.unlock_all();
    }

    /// Reads the waypoint section.
    pub fn read(r: &mut BitReader<'_>) -> Result<Self> {
        let magic = r.read_u16()?;
        if magic != Self::MAGIC {
            return Err(Error::InvalidMagic { section: "waypoint section", found: magic as u32 });
        }
        Ok(WaypointSection {
            version: r.read_u32()?,
            section_size: r.read_u16()?,
            normal: WaypointDifficulty::read(r)?,
            nightmare: WaypointDifficulty::read(r)?,
            hell: WaypointDifficulty::read(r)?,
        })
    }

    /// Writes the waypoint section.
    pub fn write(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.write_u16(Self::MAGIC)?;
        w.write_u32(self.version)?;
        w.write_u16(self.section_size)?;
        self.normal.write(w)?;
        self.nightmare.write(w)?;
        self.hell.write(w)
    }
}

/// Waypoints for one difficulty (24 bytes): version, a 112-bit waypoint field
/// (39 bits used in vanilla) and 8 reserved bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WaypointDifficulty {
    /// Waypoint record version. The game writes 0x0102 and resets all waypoints on load if it differs.
    pub version: u16,
    /// Act I waypoints.
    pub act1: ActIWaypoints,
    /// Act II waypoints.
    pub act2: ActIIWaypoints,
    /// Act III waypoints.
    pub act3: ActIIIWaypoints,
    /// Act IV waypoints.
    pub act4: ActIVWaypoints,
    /// Act V waypoints.
    pub act5: ActVWaypoints,
    /// Waypoint bits 39-47 (part of the game's 112-bit waypoint field, unused in vanilla).
    pub extra_waypoint_bits: u16,
    /// Waypoint bits 48-111 (part of the game's 112-bit waypoint field, unused in vanilla).
    pub extra_waypoint_bytes: [u8; 8],
    /// Reserved bytes (always written as zero, never read by the game).
    pub reserved: [u8; 8],
}

impl Default for WaypointDifficulty {
    fn default() -> Self {
        WaypointDifficulty {
            version: 0x0102,
            act1: ActIWaypoints::ROGUE_ENCAMPMENT,
            act2: ActIIWaypoints::empty(),
            act3: ActIIIWaypoints::empty(),
            act4: ActIVWaypoints::empty(),
            act5: ActVWaypoints::empty(),
            extra_waypoint_bits: 0,
            extra_waypoint_bytes: [0; 8],
            reserved: [0; 8],
        }
    }
}

impl WaypointDifficulty {
    /// Number of waypoints.
    pub const COUNT: usize = 39;

    /// Whether a waypoint (global index 0-38) is active.
    pub fn is_active(&self, index: usize) -> bool {
        match index {
            0..=8 => self.act1.bits() & (1 << index) != 0,
            9..=17 => self.act2.bits() & (1 << (index - 9)) != 0,
            18..=26 => self.act3.bits() & (1 << (index - 18)) != 0,
            27..=29 => self.act4.bits() & (1 << (index - 27)) != 0,
            30..=38 => self.act5.bits() & (1 << (index - 30)) != 0,
            _ => false,
        }
    }

    /// Sets or clears a waypoint (global index 0-38). Out of range indexes are ignored.
    pub fn set_active(&mut self, index: usize, active: bool) {
        fn apply<T: bitflags::Flags>(flags: &mut T, bit: T::Bits, active: bool) {
            flags.set(T::from_bits_retain(bit), active);
        }
        match index {
            0..=8 => apply(&mut self.act1, 1 << index, active),
            9..=17 => apply(&mut self.act2, 1 << (index - 9), active),
            18..=26 => apply(&mut self.act3, 1 << (index - 18), active),
            27..=29 => apply(&mut self.act4, 1 << (index - 27), active),
            30..=38 => apply(&mut self.act5, 1 << (index - 30), active),
            _ => {}
        }
    }

    /// Unlocks all waypoints.
    pub fn unlock_all(&mut self) {
        for i in 0..Self::COUNT {
            self.set_active(i, true);
        }
    }

    /// Reads waypoints for one difficulty.
    pub fn read(r: &mut BitReader<'_>) -> Result<Self> {
        Ok(WaypointDifficulty {
            version: r.read_u16()?,
            act1: ActIWaypoints::from_bits_retain(r.read_bits(9)? as u16),
            act2: ActIIWaypoints::from_bits_retain(r.read_bits(9)? as u16),
            act3: ActIIIWaypoints::from_bits_retain(r.read_bits(9)? as u16),
            act4: ActIVWaypoints::from_bits_retain(r.read_bits(3)? as u8),
            act5: ActVWaypoints::from_bits_retain(r.read_bits(9)? as u16),
            extra_waypoint_bits: r.read_bits(9)? as u16,
            extra_waypoint_bytes: r.read_array()?,
            reserved: r.read_array()?,
        })
    }

    /// Writes waypoints for one difficulty.
    pub fn write(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.write_u16(self.version)?;
        w.write_bits(self.act1.bits() as u32, 9)?;
        w.write_bits(self.act2.bits() as u32, 9)?;
        w.write_bits(self.act3.bits() as u32, 9)?;
        w.write_bits(self.act4.bits() as u32, 3)?;
        w.write_bits(self.act5.bits() as u32, 9)?;
        w.write_bits(self.extra_waypoint_bits as u32, 9)?;
        w.write_bytes(&self.extra_waypoint_bytes)?;
        w.write_bytes(&self.reserved)
    }
}

/// NPC dialog flags for a single NPC.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct NpcDialog {
    /// Whether the quest introduction has been shown.
    pub quest_intro_shown: bool,
    /// Whether the NPC introduction has been shown.
    pub npc_intro_shown: bool,
}

/// NPC indexes used by [`NpcDialogDifficulty`].
#[allow(missing_docs)]
pub mod npc {
    pub const GHEED: usize = 1;
    pub const AKARA: usize = 2;
    pub const KASHYA: usize = 3;
    pub const WARRIV_ACT1: usize = 4;
    pub const CHARSI: usize = 5;
    pub const CAIN_ACT1: usize = 6;
    pub const WARRIV_ACT2: usize = 7;
    pub const ATMA: usize = 8;
    pub const DROGNAN: usize = 9;
    pub const FARA: usize = 10;
    pub const LYSANDER: usize = 11;
    pub const GEGLASH: usize = 12;
    pub const MESHIF_ACT2: usize = 13;
    pub const JERHYN: usize = 14;
    pub const GREIZ: usize = 15;
    pub const ELZIX: usize = 16;
    pub const CAIN_ACT2: usize = 17;
    pub const CAIN_ACT3: usize = 18;
    pub const CAIN_ACT4: usize = 19;
    pub const TYRAEL: usize = 20;
    pub const ASHEARA: usize = 21;
    pub const HRATLI: usize = 22;
    pub const ALKOR: usize = 23;
    pub const ORMUS: usize = 24;
    pub const IZUAL: usize = 25;
    pub const HALBU: usize = 26;
    pub const MESHIF_ACT3: usize = 27;
    pub const NATALYA: usize = 28;
    pub const LARZUK: usize = 29;
    pub const ANYA: usize = 30;
    pub const MALAH: usize = 31;
    pub const NIHLATHAK: usize = 32;
    pub const QUAL_KEHK: usize = 33;
    pub const CAIN_ACT5: usize = 34;
}

/// NPC dialog flags for one difficulty (two 64-bit masks).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NpcDialogDifficulty {
    /// Quest intro flags, one bit per NPC index.
    pub quest_intro_flags: [u8; 8],
    /// NPC intro flags, one bit per NPC index.
    pub npc_intro_flags: [u8; 8],
}

impl NpcDialogDifficulty {
    fn bit(buf: &[u8; 8], index: usize) -> bool {
        index < 64 && buf[index / 8] & (1 << (index % 8)) != 0
    }

    fn set_bit(buf: &mut [u8; 8], index: usize, value: bool) {
        if index < 64 {
            if value {
                buf[index / 8] |= 1 << (index % 8);
            } else {
                buf[index / 8] &= !(1 << (index % 8));
            }
        }
    }

    /// Dialog state for an NPC (see [`npc`] for indexes).
    pub fn npc(&self, index: usize) -> NpcDialog {
        NpcDialog {
            quest_intro_shown: Self::bit(&self.quest_intro_flags, index),
            npc_intro_shown: Self::bit(&self.npc_intro_flags, index),
        }
    }

    /// Sets the dialog state for an NPC.
    pub fn set_npc(&mut self, index: usize, dialog: NpcDialog) {
        Self::set_bit(&mut self.quest_intro_flags, index, dialog.quest_intro_shown);
        Self::set_bit(&mut self.npc_intro_flags, index, dialog.npc_intro_shown);
    }

    /// Whether the quest intro was shown for an NPC.
    pub fn is_quest_intro_shown(&self, index: usize) -> bool {
        Self::bit(&self.quest_intro_flags, index)
    }

    /// Sets whether the quest intro was shown for an NPC.
    pub fn set_quest_intro_shown(&mut self, index: usize, value: bool) {
        Self::set_bit(&mut self.quest_intro_flags, index, value);
    }

    /// Whether the NPC intro was shown.
    pub fn is_npc_intro_shown(&self, index: usize) -> bool {
        Self::bit(&self.npc_intro_flags, index)
    }

    /// Sets whether the NPC intro was shown.
    pub fn set_npc_intro_shown(&mut self, index: usize, value: bool) {
        Self::set_bit(&mut self.npc_intro_flags, index, value);
    }
}

/// Player intro / NPC dialog section (52 bytes, magic 0x7701).
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerIntroSection {
    /// Section header (0x7701).
    pub header: u16,
    /// Section size (52).
    pub section_size: u16,
    /// Normal difficulty.
    pub normal: NpcDialogDifficulty,
    /// Nightmare difficulty.
    pub nightmare: NpcDialogDifficulty,
    /// Hell difficulty.
    pub hell: NpcDialogDifficulty,
}

impl Default for PlayerIntroSection {
    fn default() -> Self {
        PlayerIntroSection {
            header: Self::MAGIC,
            section_size: Self::SIZE as u16,
            normal: Default::default(),
            nightmare: Default::default(),
            hell: Default::default(),
        }
    }
}

impl PlayerIntroSection {
    /// Section magic.
    pub const MAGIC: u16 = 0x7701;
    /// Section size in bytes.
    pub const SIZE: usize = 52;

    /// Dialog flags for a difficulty.
    pub fn get(&self, difficulty: Difficulty) -> Option<&NpcDialogDifficulty> {
        match difficulty {
            Difficulty::Normal => Some(&self.normal),
            Difficulty::Nightmare => Some(&self.nightmare),
            Difficulty::Hell => Some(&self.hell),
            Difficulty::Unknown(_) => None,
        }
    }

    /// Mutable dialog flags for a difficulty.
    pub fn get_mut(&mut self, difficulty: Difficulty) -> Option<&mut NpcDialogDifficulty> {
        match difficulty {
            Difficulty::Normal => Some(&mut self.normal),
            Difficulty::Nightmare => Some(&mut self.nightmare),
            Difficulty::Hell => Some(&mut self.hell),
            Difficulty::Unknown(_) => None,
        }
    }

    /// Reads the section.
    pub fn read(r: &mut BitReader<'_>) -> Result<Self> {
        let header = r.read_u16()?;
        if header != Self::MAGIC {
            return Err(Error::InvalidMagic { section: "player intro section", found: header as u32 });
        }
        let section_size = r.read_u16()?;
        if section_size as usize != Self::SIZE {
            return Err(Error::invalid_data(format!(
                "failed to read player intro section, invalid size: {section_size}"
            )));
        }
        let mut s = PlayerIntroSection { header, section_size, ..Default::default() };
        r.read_bytes(&mut s.normal.quest_intro_flags)?;
        r.read_bytes(&mut s.nightmare.quest_intro_flags)?;
        r.read_bytes(&mut s.hell.quest_intro_flags)?;
        r.read_bytes(&mut s.normal.npc_intro_flags)?;
        r.read_bytes(&mut s.nightmare.npc_intro_flags)?;
        r.read_bytes(&mut s.hell.npc_intro_flags)?;
        Ok(s)
    }

    /// Writes the section.
    pub fn write(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.write_u16(self.header)?;
        w.write_u16(self.section_size)?;
        w.write_bytes(&self.normal.quest_intro_flags)?;
        w.write_bytes(&self.nightmare.quest_intro_flags)?;
        w.write_bytes(&self.hell.quest_intro_flags)?;
        w.write_bytes(&self.normal.npc_intro_flags)?;
        w.write_bytes(&self.nightmare.npc_intro_flags)?;
        w.write_bytes(&self.hell.npc_intro_flags)
    }
}
