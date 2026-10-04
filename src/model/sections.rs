//! Item container sections.

use crate::bits::{BitReader, BitWriter};
use crate::data::ExternalData;
use crate::error::{Error, Result};
use crate::model::item::Item;

macro_rules! list_newtype {
    ($name:ident, $item:ty) => {
        impl ::std::ops::Deref for $name {
            type Target = Vec<$item>;
            fn deref(&self) -> &Vec<$item> {
                &self.0
            }
        }

        impl ::std::ops::DerefMut for $name {
            fn deref_mut(&mut self) -> &mut Vec<$item> {
                &mut self.0
            }
        }

        impl<'a> IntoIterator for &'a $name {
            type Item = &'a $item;
            type IntoIter = std::slice::Iter<'a, $item>;
            fn into_iter(self) -> Self::IntoIter {
                self.0.iter()
            }
        }

        impl<'a> IntoIterator for &'a mut $name {
            type Item = &'a mut $item;
            type IntoIter = std::slice::IterMut<'a, $item>;
            fn into_iter(self) -> Self::IntoIter {
                self.0.iter_mut()
            }
        }

        impl IntoIterator for $name {
            type Item = $item;
            type IntoIter = std::vec::IntoIter<$item>;
            fn into_iter(self) -> Self::IntoIter {
                self.0.into_iter()
            }
        }

        impl FromIterator<$item> for $name {
            fn from_iter<T: IntoIterator<Item = $item>>(iter: T) -> Self {
                $name(iter.into_iter().collect())
            }
        }

        impl From<Vec<$item>> for $name {
            fn from(v: Vec<$item>) -> Self {
                $name(v)
            }
        }
    };
}

pub(crate) use list_newtype;

/// An items section ("JM" + count + items).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct ItemsSection(pub Vec<Item>);

list_newtype!(ItemsSection, Item);

impl ItemsSection {
    /// Section magic "JM".
    pub const MAGIC: u16 = 0x4D4A;

    /// Reads an items section.
    pub fn read(r: &mut BitReader<'_>, data: &(impl ExternalData + ?Sized), save_version: u32) -> Result<Self> {
        let magic = r.read_u16()?;
        if magic != Self::MAGIC {
            return Err(Error::InvalidMagic { section: "items section", found: magic as u32 });
        }
        let count = r.read_u16()?;
        let mut items = Vec::with_capacity((count as usize).min(r.bits_remaining() / 32));
        for _ in 0..count {
            items.push(Item::read(r, data, save_version)?);
        }
        Ok(ItemsSection(items))
    }

    /// Writes an items section.
    pub fn write(&self, w: &mut BitWriter<'_>, data: &(impl ExternalData + ?Sized), save_version: u32) -> Result<()> {
        w.write_u16(Self::MAGIC)?;
        w.write_u16(self.0.len() as u16)?;
        for item in &self.0 {
            item.write(w, data, save_version)?;
        }
        Ok(())
    }

    /// Conservative size estimate (200 bytes per item, including socketed items).
    pub fn estimate_size(&self) -> usize {
        4 + self.0.iter().map(Item::count_recursive).sum::<usize>() * 200
    }
}

/// A player corpse.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Corpse {
    /// Unused (never initialized by the game, so it holds stack garbage; ignored on load).
    pub unused: u32,
    /// X position.
    pub x: u32,
    /// Y position.
    pub y: u32,
    /// Items on the corpse.
    pub items: ItemsSection,
}

impl Corpse {
    /// Reads a corpse.
    pub fn read(r: &mut BitReader<'_>, data: &(impl ExternalData + ?Sized), save_version: u32) -> Result<Self> {
        Ok(Corpse {
            unused: r.read_u32()?,
            x: r.read_u32()?,
            y: r.read_u32()?,
            items: ItemsSection::read(r, data, save_version)?,
        })
    }

    /// Writes a corpse.
    pub fn write(&self, w: &mut BitWriter<'_>, data: &(impl ExternalData + ?Sized), save_version: u32) -> Result<()> {
        w.write_u32(self.unused)?;
        w.write_u32(self.x)?;
        w.write_u32(self.y)?;
        self.items.write(w, data, save_version)
    }
}

/// Corpse section ("JM" + count + corpses).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct CorpseSection(pub Vec<Corpse>);

list_newtype!(CorpseSection, Corpse);

impl CorpseSection {
    /// Section magic "JM".
    pub const MAGIC: u16 = 0x4D4A;

    /// Reads the corpse section.
    pub fn read(r: &mut BitReader<'_>, data: &(impl ExternalData + ?Sized), save_version: u32) -> Result<Self> {
        let magic = r.read_u16()?;
        if magic != Self::MAGIC {
            return Err(Error::InvalidMagic { section: "corpse section", found: magic as u32 });
        }
        let count = r.read_u16()?;
        let mut corpses = Vec::with_capacity(count.min(16) as usize);
        for _ in 0..count {
            corpses.push(Corpse::read(r, data, save_version)?);
        }
        Ok(CorpseSection(corpses))
    }

    /// Writes the corpse section.
    pub fn write(&self, w: &mut BitWriter<'_>, data: &(impl ExternalData + ?Sized), save_version: u32) -> Result<()> {
        w.write_u16(Self::MAGIC)?;
        w.write_u16(self.0.len() as u16)?;
        for corpse in &self.0 {
            corpse.write(w, data, save_version)?;
        }
        Ok(())
    }
}

/// Mercenary items section ("jf", expansion only).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MercItemsSection {
    /// Mercenary equipped items (only serialized when a mercenary exists).
    pub items: ItemsSection,
}

impl MercItemsSection {
    /// Section magic "jf".
    pub const MAGIC: u16 = 0x666A;

    /// Reads the section; the items list is only present if `has_merc`.
    pub fn read(
        r: &mut BitReader<'_>,
        data: &(impl ExternalData + ?Sized),
        has_merc: bool,
        save_version: u32,
    ) -> Result<Self> {
        let magic = r.read_u16()?;
        if magic != Self::MAGIC {
            return Err(Error::InvalidMagic { section: "merc items section", found: magic as u32 });
        }
        let items = if has_merc { ItemsSection::read(r, data, save_version)? } else { ItemsSection::default() };
        Ok(MercItemsSection { items })
    }

    /// Writes the section; the items list is only written if `has_merc`.
    pub fn write(
        &self,
        w: &mut BitWriter<'_>,
        data: &(impl ExternalData + ?Sized),
        has_merc: bool,
        save_version: u32,
    ) -> Result<()> {
        w.write_u16(Self::MAGIC)?;
        if has_merc {
            self.items.write(w, data, save_version)?;
        }
        Ok(())
    }
}

/// Iron golem section ("kf", expansion only).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IronGolemSection {
    /// The item the iron golem was created from.
    pub golem_item: Option<Item>,
}

impl IronGolemSection {
    /// Section magic "kf".
    pub const MAGIC: u16 = 0x666B;

    /// Reads the section.
    pub fn read(r: &mut BitReader<'_>, data: &(impl ExternalData + ?Sized), save_version: u32) -> Result<Self> {
        let magic = r.read_u16()?;
        if magic != Self::MAGIC {
            return Err(Error::InvalidMagic { section: "iron golem section", found: magic as u32 });
        }
        let golem_item = if r.read_u8()? != 0 { Some(Item::read(r, data, save_version)?) } else { None };
        Ok(IronGolemSection { golem_item })
    }

    /// Writes the section.
    pub fn write(&self, w: &mut BitWriter<'_>, data: &(impl ExternalData + ?Sized), save_version: u32) -> Result<()> {
        w.write_u16(Self::MAGIC)?;
        w.write_u8(self.golem_item.is_some() as u8)?;
        if let Some(item) = &self.golem_item {
            item.write(w, data, save_version)?;
        }
        Ok(())
    }
}
