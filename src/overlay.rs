//! Zero-copy views over the fixed-size header sections of save files.
//!
//! The overlay types read and write fields directly in a byte buffer without parsing
//! the whole save. Every layout is generic over its storage: use `&[u8]` (or any
//! `AsRef<[u8]>`) for read-only access and `&mut [u8]` (or `Vec<u8>`) to modify fields.
//!
//! Two save layouts exist: [`D2SaveLayout`] for versions <= 103 (765 bytes) and
//! [`D2SaveLayoutV104`] for versions >= 104 (833 bytes). [`SaveLayout::new`] picks
//! the right one automatically.
//!
//! ```no_run
//! # fn main() -> d2s::Result<()> {
//! let mut data = std::fs::read("MyCharacter.d2s")?;
//! let mut layout = d2s::overlay::SaveLayout::new(&mut data[..])?;
//! println!("{} level {}", layout.name(), layout.level());
//! layout.set_level(99);
//! layout.update_checksum();
//! # Ok(())
//! # }
//! ```

use std::borrow::Cow;

use crate::checksum;
use crate::enums::{
    CharacterClass, CharacterFlags, GameVersion, ItemFlags, ItemQuality, QuestFlags, Side, StashTabType,
};
use crate::error::{Error, Result};
use crate::model::{D2Save, D2StashTab, PlayerIntroSection, QuestSection, WaypointSection};
use crate::text;
use crate::ItemCode;

/// A fixed-size little-endian field.
trait Field: Sized {
    const SIZE: usize;
    fn get(b: &[u8]) -> Self;
    fn put(self, b: &mut [u8]);
}

macro_rules! int_field {
    ($($ty:ty),*) => {$(
        impl Field for $ty {
            const SIZE: usize = std::mem::size_of::<$ty>();
            #[inline]
            fn get(b: &[u8]) -> Self {
                <$ty>::from_le_bytes(b[..Self::SIZE].try_into().unwrap())
            }
            #[inline]
            fn put(self, b: &mut [u8]) {
                b[..Self::SIZE].copy_from_slice(&self.to_le_bytes());
            }
        }
    )*};
}
int_field!(u8, u16, u32, u64);

macro_rules! wrapped_field {
    ($ty:ty, $raw:ty, $from:expr, $to:expr) => {
        impl Field for $ty {
            const SIZE: usize = std::mem::size_of::<$raw>();
            #[inline]
            fn get(b: &[u8]) -> Self {
                $from(<$raw as Field>::get(b))
            }
            #[inline]
            fn put(self, b: &mut [u8]) {
                <$raw as Field>::put($to(self), b)
            }
        }
    };
}
wrapped_field!(CharacterFlags, u32, CharacterFlags::from_bits_retain, |f: CharacterFlags| f.bits());
wrapped_field!(ItemFlags, u32, ItemFlags::from_bits_retain, |f: ItemFlags| f.bits());
wrapped_field!(QuestFlags, u16, QuestFlags::from_bits_retain, |f: QuestFlags| f.bits());
wrapped_field!(CharacterClass, u8, CharacterClass::from_raw, CharacterClass::to_raw);
wrapped_field!(ItemQuality, u8, ItemQuality::from_raw, ItemQuality::to_raw);
wrapped_field!(GameVersion, u8, GameVersion::from_raw, GameVersion::to_raw);
wrapped_field!(StashTabType, u8, StashTabType::from_raw, StashTabType::to_raw);
wrapped_field!(ItemCode, u32, ItemCode, |c: ItemCode| c.0);

macro_rules! layout {
    ($(#[$meta:meta])* $name:ident, $size:expr) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug)]
        pub struct $name<B> {
            bytes: B,
        }

        impl<B: AsRef<[u8]>> $name<B> {
            /// Size of the layout in bytes.
            pub const SIZE: usize = $size;

            #[inline]
            #[allow(dead_code)]
            fn wrap(bytes: B) -> Self {
                debug_assert!(bytes.as_ref().len() >= Self::SIZE);
                $name { bytes }
            }

            /// The underlying bytes.
            pub fn as_bytes(&self) -> &[u8] {
                self.bytes.as_ref()
            }

            /// Returns the underlying storage.
            pub fn into_inner(self) -> B {
                self.bytes
            }

            #[inline]
            #[allow(dead_code)]
            fn get<T: Field>(&self, offset: usize) -> T {
                T::get(&self.bytes.as_ref()[offset..])
            }
        }

        impl<B: AsRef<[u8]> + AsMut<[u8]>> $name<B> {
            #[inline]
            #[allow(dead_code)]
            fn put<T: Field>(&mut self, offset: usize, value: T) {
                value.put(&mut self.bytes.as_mut()[offset..])
            }
        }
    };
}

macro_rules! fields {
    ($name:ident { $( $(#[$meta:meta])* $get:ident, $set:ident: $ty:ty = $off:expr; )* }) => {
        impl<B: AsRef<[u8]>> $name<B> {
            $( $(#[$meta])* #[inline] pub fn $get(&self) -> $ty { self.get($off) } )*
        }
        impl<B: AsRef<[u8]> + AsMut<[u8]>> $name<B> {
            $( $(#[$meta])* #[inline] pub fn $set(&mut self, value: $ty) { self.put($off, value) } )*
        }
    };
}

macro_rules! sections {
    ($name:ident { $( $(#[$meta:meta])* $get:ident, $get_mut:ident: $ty:ident = $off:expr; )* }) => {
        impl<B: AsRef<[u8]>> $name<B> {
            $(
                $(#[$meta])*
                pub fn $get(&self) -> $ty<&[u8]> {
                    $ty::wrap(&self.bytes.as_ref()[$off..$off + $ty::<&[u8]>::SIZE])
                }
            )*
        }
        impl<B: AsRef<[u8]> + AsMut<[u8]>> $name<B> {
            $(
                $(#[$meta])*
                pub fn $get_mut(&mut self) -> $ty<&mut [u8]> {
                    $ty::wrap(&mut self.bytes.as_mut()[$off..$off + $ty::<&[u8]>::SIZE])
                }
            )*
        }
    };
}

macro_rules! byte_arrays {
    ($name:ident { $( $(#[$meta:meta])* $get:ident, $get_mut:ident: $len:expr => $off:expr; )* }) => {
        impl<B: AsRef<[u8]>> $name<B> {
            $( $(#[$meta])* pub fn $get(&self) -> &[u8; $len] {
                self.bytes.as_ref()[$off..$off + $len].try_into().unwrap()
            } )*
        }
        impl<B: AsRef<[u8]> + AsMut<[u8]>> $name<B> {
            $( $(#[$meta])* pub fn $get_mut(&mut self) -> &mut [u8; $len] {
                (&mut self.bytes.as_mut()[$off..$off + $len]).try_into().unwrap()
            } )*
        }
    };
}

fn check_len(data: &[u8], size: usize, what: &str) -> Result<()> {
    if data.len() < size {
        return Err(Error::invalid_data(format!("data too small for {what}: {} bytes, need {size}", data.len())));
    }
    Ok(())
}

// --- Header -------------------------------------------------------------------------

layout!(
    /// File header (16 bytes at offset 0).
    HeaderLayout,
    16
);

fields!(HeaderLayout {
    /// File magic (0xAA55AA55).
    magic, set_magic: u32 = 0;
    /// Save version.
    version, set_version: u32 = 4;
    /// File size.
    file_size, set_file_size: u32 = 8;
    /// Checksum.
    checksum, set_checksum: u32 = 12;
});

/// Returns the save version of raw save data, validating the file magic.
pub fn save_version(data: &[u8]) -> Result<u32> {
    check_len(data, HeaderLayout::<&[u8]>::SIZE, "save header")?;
    let header = HeaderLayout::wrap(&data[..HeaderLayout::<&[u8]>::SIZE]);
    if header.magic() != D2Save::MAGIC {
        return Err(Error::InvalidMagic { section: "save file", found: header.magic() });
    }
    Ok(header.version())
}

// --- Character sub-structures -------------------------------------------------------

layout!(
    /// Mercenary data (16 bytes).
    MercDataLayout,
    16
);

fields!(MercDataLayout {
    /// Mercenary flags.
    flags, set_flags: u32 = 0;
    /// Random seed.
    seed, set_seed: u32 = 4;
    /// Name index.
    name_index, set_name_index: u16 = 8;
    /// Hireling type ID.
    hireling_id, set_hireling_id: u16 = 10;
    /// Experience.
    experience, set_experience: u32 = 12;
});

impl<B: AsRef<[u8]>> MercDataLayout<B> {
    /// Whether the mercenary is dead.
    pub fn is_dead(&self) -> bool {
        self.flags() & 0x0001_0000 != 0
    }

    /// Whether a mercenary has been hired.
    pub fn has_merc(&self) -> bool {
        self.seed() != 0 || self.name_index() != 0 || self.experience() != 0
    }
}

impl<B: AsRef<[u8]> + AsMut<[u8]>> MercDataLayout<B> {
    /// Sets whether the mercenary is dead.
    pub fn set_dead(&mut self, dead: bool) {
        let flags = if dead { self.flags() | 0x0001_0000 } else { self.flags() & !0x0001_0000 };
        self.set_flags(flags);
    }
}

layout!(
    /// Preview item (12 bytes).
    PreviewItemLayout,
    12
);

fields!(PreviewItemLayout {
    /// Item code.
    code, set_code: ItemCode = 0;
    /// Color transform.
    transform, set_transform: u8 = 4;
    /// Quality.
    quality, set_quality: ItemQuality = 5;
    /// Graphics file index.
    file_index, set_file_index: u16 = 6;
    /// Item flags.
    flags, set_flags: ItemFlags = 8;
});

layout!(
    /// Skill hotkey (4 bytes).
    SkillLayout,
    4
);

impl<B: AsRef<[u8]>> SkillLayout<B> {
    /// Skill ID (15 bits).
    pub fn skill_id(&self) -> u16 {
        self.get::<u16>(0) & 0x7FFF
    }

    /// Skill bar side.
    pub fn side(&self) -> Side {
        if self.get::<u16>(0) >> 15 != 0 {
            Side::Left
        } else {
            Side::Right
        }
    }

    /// Item index.
    pub fn item_index(&self) -> u16 {
        self.get(2)
    }
}

impl<B: AsRef<[u8]> + AsMut<[u8]>> SkillLayout<B> {
    /// Sets the skill ID.
    pub fn set_skill_id(&mut self, id: u16) {
        let raw = (self.get::<u16>(0) & 0x8000) | (id & 0x7FFF);
        self.put(0, raw);
    }

    /// Sets the skill bar side.
    pub fn set_side(&mut self, side: Side) {
        let raw = (self.get::<u16>(0) & 0x7FFF) | ((side as u16) << 15);
        self.put(0, raw);
    }

    /// Sets the item index.
    pub fn set_item_index(&mut self, index: u16) {
        self.put(2, index);
    }
}

// --- v<=103 preview and character ---------------------------------------------------

layout!(
    /// Preview data for v<=103 (144 bytes).
    PreviewDataLayout,
    144
);

fields!(PreviewDataLayout {
    /// Expansion save time.
    expansion_save_time, set_expansion_save_time: u64 = 0;
    /// Classic save time.
    classic_save_time, set_classic_save_time: u64 = 8;
    /// Guild emblem color.
    guild_emblem_color, set_guild_emblem_color: u8 = 16;
    /// Game version (the game falls back to the expansion flag when not 1-3).
    game_version, set_game_version: GameVersion = 17;
    /// Padding after the game version.
    preview_padding, set_preview_padding: u16 = 18;
    /// Expansion experience.
    expansion_experience, set_expansion_experience: u32 = 20;
    /// Classic experience.
    classic_experience, set_classic_experience: u32 = 24;
    /// Reserved bytes after the name.
    reserved, set_reserved: u64 = 136;
});

sections!(PreviewDataLayout {
    /// Left hand preview item.
    left_hand, left_hand_mut: PreviewItemLayout = 28;
    /// Right hand preview item.
    right_hand, right_hand_mut: PreviewItemLayout = 40;
    /// Torso preview item.
    torso, torso_mut: PreviewItemLayout = 52;
    /// Head preview item.
    head, head_mut: PreviewItemLayout = 64;
});

byte_arrays!(PreviewDataLayout {
    /// Raw UTF-8 name field (60 bytes).
    name_bytes, name_bytes_mut: 60 => 76;
});

layout!(
    /// Character section for v<=103 (319 bytes at offset 0x10).
    CharacterLayout,
    319
);

fields!(CharacterLayout {
    /// Active weapon set.
    weapon_switch, set_weapon_switch: u32 = 0;
    /// Character flags.
    flags, set_flags: CharacterFlags = 20;
    /// Character class.
    class, set_class: CharacterClass = 24;
    /// Number of stats.
    num_stats, set_num_stats: u8 = 25;
    /// Number of skills.
    num_skills, set_num_skills: u8 = 26;
    /// Level.
    level, set_level: u8 = 27;
    /// Creation time.
    create_time, set_create_time: u32 = 28;
    /// Last save time.
    last_save_time, set_last_save_time: u32 = 32;
    /// Play time.
    play_time, set_play_time: u32 = 36;
    /// Map seed.
    map_seed, set_map_seed: u32 = 155;
});

sections!(CharacterLayout {
    /// Left mouse skill.
    left_skill, left_skill_mut: SkillLayout = 104;
    /// Right mouse skill.
    right_skill, right_skill_mut: SkillLayout = 108;
    /// Weapon switch left mouse skill.
    switch_left_skill, switch_left_skill_mut: SkillLayout = 112;
    /// Weapon switch right mouse skill.
    switch_right_skill, switch_right_skill_mut: SkillLayout = 116;
    /// Mercenary data.
    merc, merc_mut: MercDataLayout = 159;
    /// Preview data.
    preview, preview_mut: PreviewDataLayout = 175;
});

byte_arrays!(CharacterLayout {
    /// Raw ASCII name field (16 bytes).
    name_bytes, name_bytes_mut: 16 => 4;
    /// Appearance graphics.
    appearance_graphics, appearance_graphics_mut: 16 => 120;
    /// Appearance tints.
    appearance_tints, appearance_tints_mut: 16 => 136;
    /// Town per difficulty.
    town_difficulty, town_difficulty_mut: 3 => 152;
});

// --- v>=104 preview and character ---------------------------------------------------

layout!(
    /// Preview data for v>=104 (228 bytes).
    PreviewDataLayoutV104,
    228
);

fields!(PreviewDataLayoutV104 {
    /// Guild emblem color.
    guild_emblem_color, set_guild_emblem_color: u8 = 0x48;
    /// Game version.
    game_version, set_game_version: GameVersion = 0x49;
    /// Padding after the game version.
    preview_padding, set_preview_padding: u16 = 0x4A;
    /// Reserved bytes after the name.
    reserved, set_reserved: u64 = 0xDC;
});

sections!(PreviewDataLayoutV104 {
    /// Left hand preview item.
    left_hand, left_hand_mut: PreviewItemLayout = 0x4C;
    /// Right hand preview item.
    right_hand, right_hand_mut: PreviewItemLayout = 0x58;
    /// Torso preview item.
    torso, torso_mut: PreviewItemLayout = 0x64;
    /// Head preview item.
    head, head_mut: PreviewItemLayout = 0x70;
});

byte_arrays!(PreviewDataLayoutV104 {
    /// Raw UTF-8 name field (96 bytes).
    name_bytes, name_bytes_mut: 96 => 0x7C;
});

impl<B: AsRef<[u8]>> PreviewDataLayoutV104<B> {
    /// Save time slot (0-5).
    pub fn save_time(&self, index: usize) -> u64 {
        assert!(index < 6, "save time index out of range");
        self.get(index * 8)
    }

    /// Experience slot (0-5).
    pub fn experience(&self, index: usize) -> u32 {
        assert!(index < 6, "experience index out of range");
        self.get(0x30 + index * 4)
    }
}

impl<B: AsRef<[u8]> + AsMut<[u8]>> PreviewDataLayoutV104<B> {
    /// Sets a save time slot (0-5).
    pub fn set_save_time(&mut self, index: usize, value: u64) {
        assert!(index < 6, "save time index out of range");
        self.put(index * 8, value);
    }

    /// Sets an experience slot (0-5).
    pub fn set_experience(&mut self, index: usize, value: u32) {
        assert!(index < 6, "experience index out of range");
        self.put(0x30 + index * 4, value);
    }
}

layout!(
    /// Character section for v>=104 (387 bytes at offset 0x10).
    CharacterLayoutV104,
    387
);

fields!(CharacterLayoutV104 {
    /// Active weapon set.
    weapon_switch, set_weapon_switch: u32 = 0x00;
    /// Character flags.
    flags, set_flags: CharacterFlags = 0x04;
    /// Character class.
    class, set_class: CharacterClass = 0x08;
    /// Number of stats.
    num_stats, set_num_stats: u8 = 0x09;
    /// Number of skills.
    num_skills, set_num_skills: u8 = 0x0A;
    /// Level.
    level, set_level: u8 = 0x0B;
    /// Creation time.
    create_time, set_create_time: u32 = 0x0C;
    /// Last save time.
    last_save_time, set_last_save_time: u32 = 0x10;
    /// Play time.
    play_time, set_play_time: u32 = 0x14;
    /// Map seed.
    map_seed, set_map_seed: u32 = 0x8B;
});

sections!(CharacterLayoutV104 {
    /// Left mouse skill.
    left_skill, left_skill_mut: SkillLayout = 0x58;
    /// Right mouse skill.
    right_skill, right_skill_mut: SkillLayout = 0x5C;
    /// Weapon switch left mouse skill.
    switch_left_skill, switch_left_skill_mut: SkillLayout = 0x60;
    /// Weapon switch right mouse skill.
    switch_right_skill, switch_right_skill_mut: SkillLayout = 0x64;
    /// Mercenary data.
    merc, merc_mut: MercDataLayout = 0x8F;
    /// Preview data.
    preview, preview_mut: PreviewDataLayoutV104 = 0x9F;
});

byte_arrays!(CharacterLayoutV104 {
    /// Appearance graphics.
    appearance_graphics, appearance_graphics_mut: 16 => 0x68;
    /// Appearance tints.
    appearance_tints, appearance_tints_mut: 16 => 0x78;
    /// Town per difficulty.
    town_difficulty, town_difficulty_mut: 3 => 0x88;
});

macro_rules! character_common {
    ($name:ident, $hotkeys:expr) => {
        impl<B: AsRef<[u8]>> $name<B> {
            /// Skill hotkey slot (0-15).
            pub fn skill_hotkey(&self, index: usize) -> SkillLayout<&[u8]> {
                assert!(index < 16, "hotkey index out of range");
                let off = $hotkeys + index * 4;
                SkillLayout::wrap(&self.bytes.as_ref()[off..off + 4])
            }

            /// Whether the hardcore flag is set.
            pub fn is_hardcore(&self) -> bool {
                self.flags().contains(CharacterFlags::HARDCORE)
            }

            /// Whether the dead flag is set.
            pub fn is_dead(&self) -> bool {
                self.flags().contains(CharacterFlags::DEAD)
            }

            /// Whether the expansion flag is set.
            pub fn is_expansion(&self) -> bool {
                self.flags().contains(CharacterFlags::EXPANSION)
            }

            /// Whether the ladder flag is set.
            pub fn is_ladder(&self) -> bool {
                self.flags().contains(CharacterFlags::LADDER)
            }
        }

        impl<B: AsRef<[u8]> + AsMut<[u8]>> $name<B> {
            /// Mutable skill hotkey slot (0-15).
            pub fn skill_hotkey_mut(&mut self, index: usize) -> SkillLayout<&mut [u8]> {
                assert!(index < 16, "hotkey index out of range");
                let off = $hotkeys + index * 4;
                SkillLayout::wrap(&mut self.bytes.as_mut()[off..off + 4])
            }

            fn set_flag(&mut self, flag: CharacterFlags, value: bool) {
                let mut flags = self.flags();
                flags.set(flag, value);
                self.set_flags(flags);
            }

            /// Sets the hardcore flag.
            pub fn set_hardcore(&mut self, value: bool) {
                self.set_flag(CharacterFlags::HARDCORE, value)
            }

            /// Sets the dead flag.
            pub fn set_dead(&mut self, value: bool) {
                self.set_flag(CharacterFlags::DEAD, value)
            }

            /// Sets the expansion flag.
            pub fn set_expansion(&mut self, value: bool) {
                self.set_flag(CharacterFlags::EXPANSION, value)
            }

            /// Sets the ladder flag.
            pub fn set_ladder(&mut self, value: bool) {
                self.set_flag(CharacterFlags::LADDER, value)
            }
        }
    };
}

character_common!(CharacterLayout, 40);
character_common!(CharacterLayoutV104, 0x18);

// --- Quests, waypoints, intro -------------------------------------------------------

layout!(
    /// Quest data for one difficulty (96 bytes): 8 slots for acts I-IV, 16 for act V.
    QuestsDifficultyLayout,
    96
);

impl<B: AsRef<[u8]>> QuestsDifficultyLayout<B> {
    fn slot_offset(act: usize, slot: usize) -> usize {
        let slots = if act == 5 { 16 } else { 8 };
        assert!((1..=5).contains(&act) && slot < slots, "quest act/slot out of range");
        ((act - 1) * 8 + slot) * 2
    }

    /// Quest flags by act (1-5) and slot.
    pub fn quest(&self, act: usize, slot: usize) -> QuestFlags {
        self.get(Self::slot_offset(act, slot))
    }
}

impl<B: AsRef<[u8]> + AsMut<[u8]>> QuestsDifficultyLayout<B> {
    /// Sets quest flags by act (1-5) and slot.
    pub fn set_quest(&mut self, act: usize, slot: usize, flags: QuestFlags) {
        self.put(Self::slot_offset(act, slot), flags);
    }
}

layout!(
    /// Quest section (298 bytes).
    QuestSectionLayout,
    298
);

fields!(QuestSectionLayout {
    /// Magic ("Woo!").
    magic, set_magic: u32 = 0;
    /// Section version.
    version, set_version: u32 = 4;
    /// Section size.
    section_size, set_section_size: u16 = 8;
});

sections!(QuestSectionLayout {
    /// Normal difficulty.
    normal, normal_mut: QuestsDifficultyLayout = 10;
    /// Nightmare difficulty.
    nightmare, nightmare_mut: QuestsDifficultyLayout = 106;
    /// Hell difficulty.
    hell, hell_mut: QuestsDifficultyLayout = 202;
});

layout!(
    /// Waypoints for one difficulty (24 bytes).
    WaypointDifficultyLayout,
    24
);

fields!(WaypointDifficultyLayout {
    /// Waypoint record version (0x0102); the game resets all waypoints on load if it differs.
    version, set_version: u16 = 0;
});

byte_arrays!(WaypointDifficultyLayout {
    /// Raw waypoint bitfield (112 bits; vanilla uses the first 39).
    waypoint_bits, waypoint_bits_mut: 14 => 2;
    /// Reserved bytes (always written as zero, never read by the game).
    reserved, reserved_mut: 8 => 16;
});

impl<B: AsRef<[u8]>> WaypointDifficultyLayout<B> {
    /// Whether a waypoint (global index 0-38) is active.
    pub fn is_active(&self, index: usize) -> bool {
        index <= 38 && self.waypoint_bits()[index / 8] & (1 << (index % 8)) != 0
    }
}

impl<B: AsRef<[u8]> + AsMut<[u8]>> WaypointDifficultyLayout<B> {
    /// Sets or clears a waypoint (global index 0-38).
    pub fn set_active(&mut self, index: usize, active: bool) {
        if index > 38 {
            return;
        }
        let bits = self.waypoint_bits_mut();
        if active {
            bits[index / 8] |= 1 << (index % 8);
        } else {
            bits[index / 8] &= !(1 << (index % 8));
        }
    }

    /// Unlocks all waypoints.
    pub fn unlock_all(&mut self) {
        for i in 0..39 {
            self.set_active(i, true);
        }
    }
}

layout!(
    /// Waypoint section (80 bytes).
    WaypointSectionLayout,
    80
);

fields!(WaypointSectionLayout {
    /// Magic ("WS").
    magic, set_magic: u16 = 0;
    /// Section version.
    version, set_version: u32 = 2;
    /// Section size.
    section_size, set_section_size: u16 = 6;
});

sections!(WaypointSectionLayout {
    /// Normal difficulty.
    normal, normal_mut: WaypointDifficultyLayout = 8;
    /// Nightmare difficulty.
    nightmare, nightmare_mut: WaypointDifficultyLayout = 32;
    /// Hell difficulty.
    hell, hell_mut: WaypointDifficultyLayout = 56;
});

impl<B: AsRef<[u8]> + AsMut<[u8]>> WaypointSectionLayout<B> {
    /// Unlocks all waypoints in all difficulties.
    pub fn unlock_all(&mut self) {
        self.normal_mut().unlock_all();
        self.nightmare_mut().unlock_all();
        self.hell_mut().unlock_all();
    }
}

layout!(
    /// Player intro section (52 bytes).
    PlayerIntroSectionLayout,
    52
);

fields!(PlayerIntroSectionLayout {
    /// Header (0x7701).
    header, set_header: u16 = 0;
    /// Section size.
    section_size, set_section_size: u16 = 2;
});

byte_arrays!(PlayerIntroSectionLayout {
    /// Normal quest intro flags.
    quest_intro_normal, quest_intro_normal_mut: 8 => 4;
    /// Nightmare quest intro flags.
    quest_intro_nightmare, quest_intro_nightmare_mut: 8 => 12;
    /// Hell quest intro flags.
    quest_intro_hell, quest_intro_hell_mut: 8 => 20;
    /// Normal NPC intro flags.
    npc_intro_normal, npc_intro_normal_mut: 8 => 28;
    /// Nightmare NPC intro flags.
    npc_intro_nightmare, npc_intro_nightmare_mut: 8 => 36;
    /// Hell NPC intro flags.
    npc_intro_hell, npc_intro_hell_mut: 8 => 44;
});

// --- Whole save layouts -------------------------------------------------------------

macro_rules! save_layout {
    ($name:ident, $size:expr, $character:ident, $quests:expr, $waypoints:expr, $intro:expr) => {
        layout!(
            /// Layout of the fixed save sections. Holds the whole save buffer.
            $name,
            $size
        );

        sections!($name {
            /// File header.
            header, header_mut: HeaderLayout = 0;
            /// Character section.
            character, character_mut: $character = 0x10;
            /// Quest section.
            quests, quests_mut: QuestSectionLayout = $quests;
            /// Waypoint section.
            waypoints, waypoints_mut: WaypointSectionLayout = $waypoints;
            /// Player intro section.
            player_intro, player_intro_mut: PlayerIntroSectionLayout = $intro;
        });

        impl<B: AsRef<[u8]>> $name<B> {
            fn validate_magics(&self) -> bool {
                self.header().magic() == D2Save::MAGIC
                    && self.quests().magic() == QuestSection::MAGIC
                    && self.waypoints().magic() == WaypointSection::MAGIC
                    && self.player_intro().header() == PlayerIntroSection::MAGIC
            }
        }

        impl<B: AsRef<[u8]> + AsMut<[u8]>> $name<B> {
            /// Recalculates the checksum over the whole buffer.
            pub fn update_checksum(&mut self) {
                checksum::update(self.bytes.as_mut()).expect("buffer is larger than the header");
            }
        }
    };
}

save_layout!(D2SaveLayout, 765, CharacterLayout, 0x14F, 0x279, 0x2C9);
save_layout!(D2SaveLayoutV104, 833, CharacterLayoutV104, 0x193, 0x2BD, 0x30D);

impl<B: AsRef<[u8]>> D2SaveLayout<B> {
    /// Wraps save data of version <= 103, validating section magics.
    pub fn new(data: B) -> Result<Self> {
        check_len(data.as_ref(), Self::SIZE, "save layout")?;
        let layout = D2SaveLayout { bytes: data };
        if !layout.validate_magics() {
            return Err(Error::invalid_data("invalid D2 save file: magic values do not match"));
        }
        if layout.header().version() >= 104 {
            return Err(Error::invalid_data(
                "save version >= 104 uses a different header layout, use D2SaveLayoutV104",
            ));
        }
        Ok(layout)
    }

    /// Character name: the 16-byte ASCII field for version <= 97, the UTF-8 preview name otherwise.
    pub fn name(&self) -> Cow<'_, str> {
        let character = &self.bytes.as_ref()[0x10..0x10 + CharacterLayout::<&[u8]>::SIZE];
        if self.header().version() > 97 {
            text::decode_fixed_utf8_cow(&character[175 + 76..175 + 136])
        } else {
            Cow::Owned(text::decode_fixed_ascii(&character[4..20]))
        }
    }
}

impl<B: AsRef<[u8]> + AsMut<[u8]>> D2SaveLayout<B> {
    /// Sets the character name in the version-appropriate field and clears the other one.
    ///
    /// Names are truncated to 59 (UTF-8) or 15 (ASCII) UTF-16 units; fails if the
    /// encoded name does not fit the field.
    pub fn set_name(&mut self, name: &str) -> Result<()> {
        let modern = self.header().version() > 97;
        let mut character = self.character_mut();
        if modern {
            text::write_utf8_field(character.preview_mut().name_bytes_mut(), &text::truncate_utf16(name, 59))?;
            character.name_bytes_mut().fill(0);
        } else {
            text::write_ascii_field(character.name_bytes_mut(), &text::truncate_utf16(name, 15))?;
            character.preview_mut().name_bytes_mut().fill(0);
        }
        Ok(())
    }
}

impl<B: AsRef<[u8]>> D2SaveLayoutV104<B> {
    /// Wraps save data of version >= 104, validating section magics.
    pub fn new(data: B) -> Result<Self> {
        check_len(data.as_ref(), Self::SIZE, "save layout")?;
        let layout = D2SaveLayoutV104 { bytes: data };
        if !layout.validate_magics() || layout.header().version() < 104 {
            return Err(Error::invalid_data("invalid D2 save file: magic values do not match or version < 104"));
        }
        Ok(layout)
    }

    /// Character name (UTF-8 preview name).
    pub fn name(&self) -> Cow<'_, str> {
        let start = 0x10 + 0x9F + 0x7C;
        text::decode_fixed_utf8_cow(&self.bytes.as_ref()[start..start + 96])
    }
}

impl<B: AsRef<[u8]> + AsMut<[u8]>> D2SaveLayoutV104<B> {
    /// Sets the character name (truncated to 95 UTF-16 units; fails if it does not fit).
    pub fn set_name(&mut self, name: &str) -> Result<()> {
        text::write_utf8_field(self.character_mut().preview_mut().name_bytes_mut(), &text::truncate_utf16(name, 95))
    }
}

/// A save layout for either header format, chosen from the save version.
#[derive(Debug)]
pub enum SaveLayout<B> {
    /// Version <= 103.
    V103(D2SaveLayout<B>),
    /// Version >= 104.
    V104(D2SaveLayoutV104<B>),
}

macro_rules! dispatch {
    ($self:ident, $l:ident => $e:expr) => {
        match $self {
            SaveLayout::V103($l) => $e,
            SaveLayout::V104($l) => $e,
        }
    };
}

impl<B: AsRef<[u8]>> SaveLayout<B> {
    /// Wraps save data, picking the layout for its version.
    pub fn new(data: B) -> Result<Self> {
        if save_version(data.as_ref())? >= 104 {
            Ok(SaveLayout::V104(D2SaveLayoutV104::new(data)?))
        } else {
            Ok(SaveLayout::V103(D2SaveLayout::new(data)?))
        }
    }

    /// Save version.
    pub fn version(&self) -> u32 {
        dispatch!(self, l => l.header().version())
    }

    /// Character name.
    pub fn name(&self) -> Cow<'_, str> {
        dispatch!(self, l => l.name())
    }

    /// Character level.
    pub fn level(&self) -> u8 {
        dispatch!(self, l => l.character().level())
    }

    /// Character class.
    pub fn class(&self) -> CharacterClass {
        dispatch!(self, l => l.character().class())
    }

    /// Character flags.
    pub fn flags(&self) -> CharacterFlags {
        dispatch!(self, l => l.character().flags())
    }

    /// Mercenary data.
    pub fn merc(&self) -> MercDataLayout<&[u8]> {
        dispatch!(self, l => l.character().into_merc())
    }

    /// Quest section.
    pub fn quests(&self) -> QuestSectionLayout<&[u8]> {
        dispatch!(self, l => l.quests())
    }

    /// Waypoint section.
    pub fn waypoints(&self) -> WaypointSectionLayout<&[u8]> {
        dispatch!(self, l => l.waypoints())
    }

    /// Player intro section.
    pub fn player_intro(&self) -> PlayerIntroSectionLayout<&[u8]> {
        dispatch!(self, l => l.player_intro())
    }
}

impl<B: AsRef<[u8]> + AsMut<[u8]>> SaveLayout<B> {
    /// Sets the character name.
    pub fn set_name(&mut self, name: &str) -> Result<()> {
        dispatch!(self, l => l.set_name(name))
    }

    /// Sets the character level.
    pub fn set_level(&mut self, level: u8) {
        dispatch!(self, l => l.character_mut().set_level(level))
    }

    /// Sets the character flags.
    pub fn set_flags(&mut self, flags: CharacterFlags) {
        dispatch!(self, l => l.character_mut().set_flags(flags))
    }

    /// Mutable mercenary data.
    pub fn merc_mut(&mut self) -> MercDataLayout<&mut [u8]> {
        dispatch!(self, l => l.character_mut().into_merc())
    }

    /// Mutable quest section.
    pub fn quests_mut(&mut self) -> QuestSectionLayout<&mut [u8]> {
        dispatch!(self, l => l.quests_mut())
    }

    /// Mutable waypoint section.
    pub fn waypoints_mut(&mut self) -> WaypointSectionLayout<&mut [u8]> {
        dispatch!(self, l => l.waypoints_mut())
    }

    /// Mutable player intro section.
    pub fn player_intro_mut(&mut self) -> PlayerIntroSectionLayout<&mut [u8]> {
        dispatch!(self, l => l.player_intro_mut())
    }

    /// Recalculates the checksum.
    pub fn update_checksum(&mut self) {
        dispatch!(self, l => l.update_checksum())
    }
}

macro_rules! into_merc {
    ($name:ident, $off:expr) => {
        impl<'a> $name<&'a [u8]> {
            fn into_merc(self) -> MercDataLayout<&'a [u8]> {
                MercDataLayout::wrap(&self.bytes[$off..$off + 16])
            }
        }
        impl<'a> $name<&'a mut [u8]> {
            fn into_merc(self) -> MercDataLayout<&'a mut [u8]> {
                MercDataLayout::wrap(&mut self.bytes[$off..$off + 16])
            }
        }
    };
}
into_merc!(CharacterLayout, 159);
into_merc!(CharacterLayoutV104, 0x8F);

// --- Stash tab ----------------------------------------------------------------------

layout!(
    /// Shared stash tab header (64 bytes).
    D2StashTabLayout,
    64
);

fields!(D2StashTabLayout {
    /// Magic (0xAA55AA55).
    magic, set_magic: u32 = 0;
    /// Stash format.
    stash_format, set_stash_format: u32 = 4;
    /// Save version of the items in the tab.
    save_version, set_save_version: u32 = 8;
    /// Gold.
    gold, set_gold: u32 = 12;
    /// Tab size including the header.
    tab_size, set_tab_size: u16 = 16;
    /// Season.
    season, set_season: u16 = 18;
    /// Tab type.
    tab_type, set_tab_type: StashTabType = 20;
});

byte_arrays!(D2StashTabLayout {
    /// Padding after the tab type.
    padding_21, padding_21_mut: 3 => 21;
    /// Opaque withdraw-only data (see [`D2StashTab::withdraw_only_data`]).
    withdraw_only_data, withdraw_only_data_mut: 20 => 24;
    /// Padding at the end of the header.
    padding_44, padding_44_mut: 20 => 44;
});

impl<B: AsRef<[u8]>> D2StashTabLayout<B> {
    /// Wraps a stash tab header, validating size and magic.
    pub fn new(data: B) -> Result<Self> {
        check_len(data.as_ref(), Self::SIZE, "stash tab header")?;
        let layout = D2StashTabLayout { bytes: data };
        if layout.magic() != D2StashTab::MAGIC {
            return Err(Error::InvalidMagic { section: "stash tab", found: layout.magic() });
        }
        Ok(layout)
    }

    /// Whether the tab is withdraw-only.
    pub fn is_withdraw_only(&self) -> bool {
        self.withdraw_only_data()[..4] != [0; 4]
    }
}
