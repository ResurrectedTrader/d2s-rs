use crate::bits::{BitReader, BitWriter};
use crate::data::ExternalData;
use crate::enums::{BodyLocation, ItemMode, StashTabType};
use crate::error::{Error, Result};
use crate::model::sections::{list_newtype, ItemsSection};

/// A D2R shared stash file (.d2i): a sequence of stash tabs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct D2StashSave(pub Vec<D2StashTab>);

list_newtype!(D2StashSave, D2StashTab);

impl D2StashSave {
    /// Reads a stash file using the embedded game data.
    #[cfg(feature = "embedded-data")]
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        Self::from_bytes_with(data, crate::TxtData::embedded())
    }

    /// Reads a stash file using the given game data.
    pub fn from_bytes_with(data: &[u8], external: &(impl ExternalData + ?Sized)) -> Result<Self> {
        Self::read(&mut BitReader::new(data), external)
    }

    /// Reads stash tabs until the remaining data is too short for a tab header.
    pub fn read(r: &mut BitReader<'_>, data: &(impl ExternalData + ?Sized)) -> Result<Self> {
        let mut tabs = Vec::new();
        while r.bits_remaining() > D2StashTab::HEADER_SIZE * 8 {
            tabs.push(D2StashTab::read(r, data)?);
        }
        Ok(D2StashSave(tabs))
    }

    /// Converts every tab to the given item format version (see [`D2StashTab::convert_to`]).
    pub fn convert_to(&mut self, version: u32) {
        for tab in &mut self.0 {
            tab.convert_to(version);
        }
    }

    /// Serializes using the embedded game data.
    #[cfg(feature = "embedded-data")]
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.to_bytes_with(crate::TxtData::embedded())
    }

    /// Serializes using the given game data.
    pub fn to_bytes_with(&self, data: &(impl ExternalData + ?Sized)) -> Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(self.estimate_size());
        self.write(&mut BitWriter::new(&mut buf), data)?;
        Ok(buf)
    }

    /// Writes all tabs.
    pub fn write(&self, w: &mut BitWriter<'_>, data: &(impl ExternalData + ?Sized)) -> Result<()> {
        for tab in &self.0 {
            tab.write(w, data)?;
        }
        Ok(())
    }

    /// Conservative estimate of the serialized size.
    pub fn estimate_size(&self) -> usize {
        self.0.iter().map(D2StashTab::estimate_size).sum()
    }
}

/// A single shared stash tab.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct D2StashTab {
    /// Stash format.
    pub stash_format: u32,
    /// Save version used to serialize the items in this tab (same as the .d2s save version).
    pub save_version: u32,
    /// Gold stored in the tab.
    pub gold: u32,
    /// Season number (create_season stat, 0 for non-seasonal).
    pub season: u16,
    /// Tab type (stash format >= 2; forced to Normal otherwise).
    pub tab_type: StashTabType,
    /// Padding after the tab type (always zero).
    pub padding_21: [u8; 3],
    /// Opaque withdraw-only data. When the first `u32` is non-zero the game only allows
    /// taking items out of the tab, and deletes the tab once it holds no gold and no items.
    /// The game never creates this data itself, it only preserves it.
    pub withdraw_only_data: [u8; 20],
    /// Padding at the end of the header (always zero).
    pub padding_44: [u8; 20],
    /// Items (Normal and AdvancedStash tabs).
    pub items: ItemsSection,
    /// Chronicle data (Chronicle tabs).
    pub chronicle: Option<ChronicleSection>,
}

impl Default for D2StashTab {
    fn default() -> Self {
        D2StashTab {
            stash_format: 0,
            save_version: 0,
            gold: 0,
            season: 0,
            tab_type: StashTabType::Normal,
            padding_21: [0; 3],
            withdraw_only_data: [0; 20],
            padding_44: [0; 20],
            items: ItemsSection::default(),
            chronicle: None,
        }
    }
}

impl D2StashTab {
    /// Header size in bytes.
    pub const HEADER_SIZE: usize = 64;
    /// Tab magic.
    pub const MAGIC: u32 = 0xAA55AA55;

    /// Whether the tab is withdraw-only (items can be taken out but not put in).
    pub fn is_withdraw_only(&self) -> bool {
        self.withdraw_only_data[..4] != [0; 4]
    }

    /// Reads a single tab using the embedded game data.
    #[cfg(feature = "embedded-data")]
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        Self::read(&mut BitReader::new(data), crate::TxtData::embedded())
    }

    /// Reads a single tab.
    pub fn read(r: &mut BitReader<'_>, data: &(impl ExternalData + ?Sized)) -> Result<Self> {
        let start = r.byte_position();
        let magic = r.read_u32()?;
        if magic != Self::MAGIC {
            return Err(Error::InvalidMagic { section: "stash tab", found: magic });
        }
        let mut tab = D2StashTab {
            stash_format: r.read_u32()?,
            save_version: r.read_u32()?,
            gold: r.read_u32()?,
            ..Default::default()
        };
        let size = r.read_u16()? as usize;
        tab.season = r.read_u16()?;
        tab.tab_type = StashTabType::from_raw(r.read_u8()?);
        tab.padding_21 = r.read_array()?;
        tab.withdraw_only_data = r.read_array()?;
        tab.padding_44 = r.read_array()?;

        // The game ignores the tab type for stash format < 2.
        if tab.stash_format < 2 {
            tab.tab_type = StashTabType::Normal;
        }

        let body_size = size as isize - Self::HEADER_SIZE as isize;
        match tab.tab_type {
            StashTabType::Chronicle => tab.chronicle = Some(ChronicleSection::read(r, body_size)?),
            _ => tab.items = ItemsSection::read(r, data, tab.save_version)?,
        }

        if size == 0 {
            return Err(Error::invalid_data("stash tab size is zero"));
        }
        r.set_byte_position(start + size);
        Ok(tab)
    }

    /// Prepares the tab for writing with a different save version and updates `save_version`.
    ///
    /// Upgrading from 1.14 clears body locations of non-equipped items; targets >= 100
    /// raise the stash format to 2, pre-100 D2R targets lower it to 1 (Normal tab type).
    pub fn convert_to(&mut self, version: u32) {
        if self.save_version <= 96 && version > 96 {
            for item in &mut self.items {
                if item.position.mode != ItemMode::Equipped {
                    item.position.body_location = BodyLocation::None;
                }
            }
        }
        (self.stash_format, self.tab_type) = Self::normalized_format(version, self.stash_format, self.tab_type);
        self.save_version = version;
    }

    /// Stash format and tab type valid for a save version: v100+ needs stash format 2,
    /// pre-100 D2R tabs use stash format 1 (which has no tab types).
    fn normalized_format(version: u32, stash_format: u32, tab_type: StashTabType) -> (u32, StashTabType) {
        if version >= 100 {
            (stash_format.max(2), tab_type)
        } else if version > 96 && stash_format >= 2 {
            (1, StashTabType::Normal)
        } else {
            (stash_format, tab_type)
        }
    }

    /// Serializes the tab using the embedded game data.
    #[cfg(feature = "embedded-data")]
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.to_bytes_with(crate::TxtData::embedded())
    }

    /// Serializes the tab using the given game data.
    pub fn to_bytes_with(&self, data: &(impl ExternalData + ?Sized)) -> Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(self.estimate_size());
        self.write(&mut BitWriter::new(&mut buf), data)?;
        Ok(buf)
    }

    /// Writes the tab. The stash format and tab type are normalized for `save_version`
    /// (as [`convert_to`](Self::convert_to) does); chronicle tabs are skipped entirely
    /// when the stash format is < 2.
    pub fn write(&self, w: &mut BitWriter<'_>, data: &(impl ExternalData + ?Sized)) -> Result<()> {
        let (stash_format, tab_type) = Self::normalized_format(self.save_version, self.stash_format, self.tab_type);
        if tab_type == StashTabType::Chronicle && stash_format < 2 {
            return Ok(());
        }
        let start = w.bytes_written();
        w.write_u32(Self::MAGIC)?;
        w.write_u32(stash_format)?;
        w.write_u32(self.save_version)?;
        w.write_u32(self.gold)?;
        let size_offset = w.byte_position();
        w.write_u16(0)?;
        w.write_u16(self.season)?;
        w.write_u8(tab_type.to_raw())?;
        w.write_bytes(&self.padding_21)?;
        w.write_bytes(&self.withdraw_only_data)?;
        w.write_bytes(&self.padding_44)?;
        match tab_type {
            StashTabType::Chronicle => {
                if let Some(chronicle) = &self.chronicle {
                    chronicle.write(w)?;
                }
            }
            _ => self.items.write(w, data, self.save_version)?,
        }
        let size = w.bytes_written() - start;
        w.patch_u16(size_offset, size as u16);
        Ok(())
    }

    /// Conservative estimate of the serialized size.
    pub fn estimate_size(&self) -> usize {
        let mut size = Self::HEADER_SIZE;
        match (&self.tab_type, &self.chronicle) {
            (StashTabType::Chronicle, Some(c)) => {
                size += 4 + 2 + 6 + 8;
                size += (c.set_entries.len() + c.unique_entries.len() + c.runeword_entries.len()) * 10;
                size += c.trailing_data.len();
            }
            _ => size += self.items.estimate_size(),
        }
        (size as f64 * 1.2) as usize
    }
}

/// Chronicle section of a chronicle tab: tracks found set/unique/runeword items.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChronicleSection {
    /// Format version (1).
    pub version: u16,
    /// Set item entries.
    pub set_entries: Vec<ChronicleEntry>,
    /// Unique item entries.
    pub unique_entries: Vec<ChronicleEntry>,
    /// Runeword entries.
    pub runeword_entries: Vec<ChronicleEntry>,
    /// Reserved bytes after the counts (always written as zero, never read by the game).
    pub reserved: [u8; 8],
    /// Bytes after the entries. The game writer includes 64 bytes of stale buffer
    /// data due to a size calculation bug; they are preserved for exact round-trips.
    pub trailing_data: Vec<u8>,
}

impl Default for ChronicleSection {
    fn default() -> Self {
        ChronicleSection {
            version: 1,
            set_entries: Vec::new(),
            unique_entries: Vec::new(),
            runeword_entries: Vec::new(),
            reserved: [0; 8],
            trailing_data: Vec::new(),
        }
    }
}

/// A chronicle entry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChronicleEntry {
    /// Item row for set/unique items, runeword string index for runewords.
    pub item_id: u32,
    /// String table ID of the name of whatever dropped the item (monster, super unique or
    /// object). 0 for runewords; the game writes 0x6A5D for an unknown spawner.
    pub source: u16,
    /// When the item was found, in minutes since the Unix epoch (UTC).
    pub timestamp: u32,
}

impl ChronicleEntry {
    /// The timestamp as a [`std::time::SystemTime`].
    pub fn time(&self) -> std::time::SystemTime {
        std::time::UNIX_EPOCH + std::time::Duration::from_secs(self.timestamp as u64 * 60)
    }
}

impl ChronicleSection {
    /// Section magic.
    pub const MAGIC: u32 = 0xC0EAEDC0;

    /// Reads the section from a tab body of `body_size` bytes.
    pub fn read(r: &mut BitReader<'_>, body_size: isize) -> Result<Self> {
        let start = r.byte_position();
        let magic = r.read_u32()?;
        if magic != Self::MAGIC {
            return Err(Error::InvalidMagic { section: "chronicle", found: magic });
        }
        let version = r.read_u16()?;
        let set_count = r.read_u16()?;
        let unique_count = r.read_u16()?;
        let runeword_count = r.read_u16()?;
        let reserved = r.read_array()?;
        let mut read_entries = |count: u16| -> Result<Vec<ChronicleEntry>> {
            (0..count)
                .map(|_| Ok(ChronicleEntry { item_id: r.read_u32()?, source: r.read_u16()?, timestamp: r.read_u32()? }))
                .collect()
        };
        let set_entries = read_entries(set_count)?;
        let unique_entries = read_entries(unique_count)?;
        let runeword_entries = read_entries(runeword_count)?;
        let consumed = (r.byte_position() - start) as isize;
        let trailing = body_size - consumed;
        let trailing_data = if trailing > 0 { r.read_slice(trailing as usize)?.to_vec() } else { Vec::new() };
        Ok(ChronicleSection { version, set_entries, unique_entries, runeword_entries, reserved, trailing_data })
    }

    /// Writes the section.
    pub fn write(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.write_u32(Self::MAGIC)?;
        w.write_u16(self.version)?;
        w.write_u16(self.set_entries.len() as u16)?;
        w.write_u16(self.unique_entries.len() as u16)?;
        w.write_u16(self.runeword_entries.len() as u16)?;
        w.write_bytes(&self.reserved)?;
        for e in self.set_entries.iter().chain(&self.unique_entries).chain(&self.runeword_entries) {
            w.write_u32(e.item_id)?;
            w.write_u16(e.source)?;
            w.write_u32(e.timestamp)?;
        }
        w.write_bytes(&self.trailing_data)
    }
}
