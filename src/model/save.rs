use crate::bits::{BitReader, BitWriter};
use crate::checksum;
use crate::data::ExternalData;
use crate::enums::{BodyLocation, CharacterFlags, Composit, GameVersion, ItemFlags, ItemMode};
use crate::error::{Error, Result};
use crate::model::character::{Character, PreviewItem};
use crate::model::demon::DemonSection;
use crate::model::item::Item;
use crate::model::progress::{PlayerIntroSection, QuestSection, WaypointSection};
use crate::model::sections::{CorpseSection, IronGolemSection, ItemsSection, MercItemsSection};
use crate::model::stats::{PlayerStats, SkillsSection};
use crate::text;

/// A Diablo 2 character save file (.d2s).
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct D2Save {
    /// Save file version.
    pub version: u32,
    /// File size as read (recomputed on write).
    pub file_size: u32,
    /// Checksum as read (recomputed on write).
    pub checksum: u32,
    /// Character section.
    pub character: Character,
    /// Quest section.
    pub quests: QuestSection,
    /// Waypoint section.
    pub waypoints: WaypointSection,
    /// NPC introduction section.
    pub player_intro: PlayerIntroSection,
    /// Player stats.
    pub stats: PlayerStats,
    /// Skill levels.
    pub skills: SkillsSection,
    /// Player items.
    pub items: ItemsSection,
    /// Corpses.
    pub corpses: CorpseSection,
    /// Mercenary items (expansion only).
    pub merc_items: Option<MercItemsSection>,
    /// Iron golem (expansion only).
    pub iron_golem: Option<IronGolemSection>,
    /// Bound demon (v>=103, Reign of the Warlock).
    pub demon: Option<DemonSection>,
    /// Bytes after the last known section (mod extensions), preserved on write.
    pub trailing_data: Vec<u8>,
}

impl Default for D2Save {
    fn default() -> Self {
        D2Save {
            version: Self::CURRENT_VERSION,
            file_size: 0,
            checksum: 0,
            character: Character::default(),
            quests: QuestSection::default(),
            waypoints: WaypointSection::default(),
            player_intro: PlayerIntroSection::default(),
            stats: PlayerStats::default(),
            skills: SkillsSection::default(),
            items: ItemsSection::default(),
            corpses: CorpseSection::default(),
            merc_items: None,
            iron_golem: None,
            demon: None,
            trailing_data: Vec::new(),
        }
    }
}

impl D2Save {
    /// File magic.
    pub const MAGIC: u32 = 0xAA55AA55;
    /// Latest supported save version.
    pub const CURRENT_VERSION: u32 = 105;

    /// Parses a save file using the embedded game data.
    #[cfg(feature = "embedded-data")]
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        Self::from_bytes_with(data, crate::TxtData::embedded())
    }

    /// Parses a save file using the given game data.
    pub fn from_bytes_with(data: &[u8], external: &(impl ExternalData + ?Sized)) -> Result<Self> {
        let mut r = BitReader::new(data);
        let magic = r.read_u32()?;
        if magic != Self::MAGIC {
            return Err(Error::InvalidMagic { section: "save file", found: magic });
        }
        let version = r.read_u32()?;
        let file_size = r.read_u32()?;
        let checksum = r.read_u32()?;

        let character = Character::read(&mut r, version)?;
        let quests = QuestSection::read(&mut r)?;
        let waypoints = WaypointSection::read(&mut r)?;
        let player_intro = PlayerIntroSection::read(&mut r)?;
        let stats = PlayerStats::read(&mut r, external, version)?;
        let skills = SkillsSection::read(&mut r, character.class, character.num_skills as usize)?;
        let items = ItemsSection::read(&mut r, external, version)?;
        let corpses = CorpseSection::read(&mut r, external, version)?;

        let mut save = D2Save {
            version,
            file_size,
            checksum,
            character,
            quests,
            waypoints,
            player_intro,
            stats,
            skills,
            items,
            corpses,
            ..Default::default()
        };

        if save.is_expansion() {
            save.merc_items = Some(MercItemsSection::read(&mut r, external, save.character.merc.has_merc(), version)?);
            save.iron_golem = Some(IronGolemSection::read(&mut r, external, version)?);
            if save.character.preview.game_version == GameVersion::ReignOfTheWarlock {
                save.demon = DemonSection::try_read(&mut r, external, version)?;
            }
        }

        let remaining = r.bits_remaining() >> 3;
        if remaining > 0 {
            save.trailing_data = r.read_slice(remaining)?.to_vec();
        }
        Ok(save)
    }

    /// Serializes using the embedded game data at the current [`version`](Self::version).
    #[cfg(feature = "embedded-data")]
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.to_bytes_with(crate::TxtData::embedded())
    }

    /// Serializes using the given game data at the current [`version`](Self::version).
    pub fn to_bytes_with(&self, data: &(impl ExternalData + ?Sized)) -> Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(self.estimate_size());
        self.write_to(&mut buf, data)?;
        Ok(buf)
    }

    /// Appends the serialized save to `buf` (allowing buffer reuse). Returns the number of bytes written.
    pub fn write_to(&self, buf: &mut Vec<u8>, data: &(impl ExternalData + ?Sized)) -> Result<usize> {
        let start = buf.len();
        let v = self.version;
        let mut w = BitWriter::new(buf);
        w.write_u32(Self::MAGIC)?;
        w.write_u32(v)?;
        w.write_u32(0)?; // file size, patched below
        w.write_u32(0)?; // checksum, patched below

        self.character.write(&mut w, v)?;
        self.quests.write(&mut w)?;
        self.waypoints.write(&mut w)?;
        self.player_intro.write(&mut w)?;
        self.stats.write(&mut w, data, v)?;
        self.skills.write(&mut w)?;
        self.items.write(&mut w, data, v)?;
        self.corpses.write(&mut w, data, v)?;

        if self.is_expansion() {
            let has_merc = self.character.merc.has_merc();
            match &self.merc_items {
                Some(merc) => merc.write(&mut w, data, has_merc, v)?,
                None => MercItemsSection::default().write(&mut w, data, has_merc, v)?,
            }
            match &self.iron_golem {
                Some(golem) => golem.write(&mut w, data, v)?,
                None => IronGolemSection::default().write(&mut w, data, v)?,
            }
            if self.character.preview.game_version == GameVersion::ReignOfTheWarlock {
                if let Some(demon) = &self.demon {
                    demon.write(&mut w, data, v)?;
                }
            }
        }

        w.write_bytes(&self.trailing_data)?;
        let len = w.bytes_written();
        w.patch_u32(8, len as u32);
        checksum::update(&mut buf[start..start + len])?;
        Ok(len)
    }

    /// Converts the save in place to another format version.
    ///
    /// Handles the 1.14 (96) / D2R (97+) boundary, the v<=103 / v>=104 header
    /// boundary and the v105 item quantity format (which needs no model changes),
    /// then sets [`version`](Self::version) so the next write uses the new format.
    pub fn convert_to(&mut self, version: u32) {
        let source = self.version;
        if source <= 96 && version > 96 {
            self.convert_114_to_d2r();
        } else if source > 96 && version <= 96 {
            self.convert_d2r_to_114();
        }
        if source < 104 && version >= 104 {
            self.convert_103_to_104();
        } else if source >= 104 && version < 104 {
            self.convert_104_to_103();
        }
        self.version = version;
    }

    /// Conservative estimate of the serialized size.
    pub fn estimate_size(&self) -> usize {
        let mut size = 16 + Character::size(self.version) + 298 + 80 + 52;
        size += 50; // stats
        size += 2 + self.character.num_skills as usize;
        size += self.items.estimate_size();
        size += 4;
        for corpse in &self.corpses {
            size += 12 + corpse.items.estimate_size();
        }
        if self.is_expansion() {
            size += 2 + self.merc_items.as_ref().map_or(4, |m| m.items.estimate_size());
            size += 3;
            if self.iron_golem.as_ref().is_some_and(|g| g.golem_item.is_some()) {
                size += 200;
            }
            if self.character.preview.game_version == GameVersion::ReignOfTheWarlock {
                if let Some(demon) = &self.demon {
                    size += 6;
                    if let Some(d) = &demon.demon {
                        size += 92 + 4 + d.stats.len() * 8;
                    }
                }
            }
        }
        size += self.trailing_data.len();
        (size as f64 * 1.2) as usize
    }

    /// Verifies the checksum of raw save data.
    pub fn verify_checksum(data: &[u8]) -> bool {
        checksum::verify(data)
    }

    /// Whether this is an expansion character (game version for v104+, flags otherwise).
    pub fn is_expansion(&self) -> bool {
        if self.version >= 104 {
            self.character.preview.game_version > GameVersion::Classic
        } else {
            self.character.flags.contains(CharacterFlags::EXPANSION)
        }
    }

    /// The character name, wherever the current version stores it
    /// (character section up to v97, UTF-8 preview name from v98).
    pub fn name(&self) -> &str {
        if self.version > 97 {
            &self.character.preview.name
        } else {
            &self.character.name
        }
    }

    fn convert_114_to_d2r(&mut self) {
        let character = &mut self.character;
        character.preview.name = std::mem::take(&mut character.name);

        // Preview left/right hands are swapped relative to body locations.
        for item in self.items.iter().filter(|i| i.position.mode == ItemMode::Equipped) {
            let tints = &character.appearance_tints;
            let preview = &mut character.preview;
            match item.position.body_location {
                BodyLocation::RightArm => {
                    preview.left_hand = preview_item(item, tints, Composit::LeftHand, ItemFlags::TARGETING)
                }
                BodyLocation::LeftArm => {
                    preview.right_hand = preview_item(item, tints, Composit::RightHand, ItemFlags::empty())
                }
                BodyLocation::Torso => preview.torso = preview_item(item, tints, Composit::Torso, ItemFlags::empty()),
                BodyLocation::Head => preview.head = preview_item(item, tints, Composit::Head, ItemFlags::empty()),
                _ => {}
            }
        }

        // D2R rejects body locations on non-equipped items.
        for item in self.items.iter_mut() {
            if item.position.mode != ItemMode::Equipped {
                item.position.body_location = BodyLocation::None;
            }
        }
    }

    fn convert_d2r_to_114(&mut self) {
        let character = &mut self.character;
        character.name = std::mem::take(&mut character.preview.name);
        character.preview.left_hand = PreviewItem::default();
        character.preview.right_hand = PreviewItem::default();
        character.preview.torso = PreviewItem::default();
        character.preview.head = PreviewItem::default();
    }

    fn convert_103_to_104(&mut self) {
        let character = &mut self.character;
        if !character.name.is_empty() {
            character.preview.name = std::mem::take(&mut character.name);
        }
        // Same rule as the game's v<=103 reader: keep a valid stored game version (1-3),
        // otherwise derive it from the expansion flag.
        if !matches!(
            character.preview.game_version,
            GameVersion::Classic | GameVersion::Expansion | GameVersion::ReignOfTheWarlock
        ) {
            character.preview.game_version = if character.flags.contains(CharacterFlags::EXPANSION) {
                GameVersion::Expansion
            } else {
                GameVersion::Classic
            };
        }
        character.flags.remove(CharacterFlags::EXPANSION);
    }

    fn convert_104_to_103(&mut self) {
        let character = &mut self.character;
        if character.name.is_empty() && !character.preview.name.is_empty() {
            character.name = text::truncate_utf16(&character.preview.name, 15).into_owned();
        }
        if character.preview.game_version > GameVersion::Classic {
            character.flags.insert(CharacterFlags::EXPANSION);
        }
        for i in 2..6 {
            character.preview.save_times[i] = 0;
            character.preview.experiences[i] = 0;
        }
    }
}

fn preview_item(item: &Item, tints: &[u8; 16], composit: Composit, flags: ItemFlags) -> PreviewItem {
    PreviewItem {
        code: item.code,
        quality: item.quality,
        flags,
        transform: tints[composit as usize],
        file_index: item.quality_data.as_ref().and_then(|q| q.file_index()).unwrap_or(0),
    }
}
