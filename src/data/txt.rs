//! External data loaded from the original game .txt files.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};
use std::path::Path;
use std::sync::OnceLock;

use super::{ExternalData, ItemTypeInfo, StatInfo};
use crate::error::{Error, Result};
use crate::{ItemCode, StatId};

/// The txt files making up one version's data set:
/// (itemtypes, itemstatcost, weapons, armor, misc).
type FileSet = [&'static str; 5];

#[cfg(feature = "embedded-data")]
macro_rules! embedded_set {
    ($dir:literal, $types:literal, $isc:literal, $weapons:literal, $armor:literal, $misc:literal) => {
        [
            include_str!(concat!("../../data/txt/", $dir, "/", $types)),
            include_str!(concat!("../../data/txt/", $dir, "/", $isc)),
            include_str!(concat!("../../data/txt/", $dir, "/", $weapons)),
            include_str!(concat!("../../data/txt/", $dir, "/", $armor)),
            include_str!(concat!("../../data/txt/", $dir, "/", $misc)),
        ]
    };
}

#[cfg(feature = "embedded-data")]
static EMBEDDED_96: FileSet =
    embedded_set!("96", "ItemTypes.txt", "ItemStatCost.txt", "weapons.txt", "armor.txt", "misc.txt");
#[cfg(feature = "embedded-data")]
static EMBEDDED_97: FileSet =
    embedded_set!("97", "itemtypes.txt", "ItemStatCost.txt", "Weapons.txt", "Armor.txt", "Misc.txt");
#[cfg(feature = "embedded-data")]
static EMBEDDED_99: FileSet =
    embedded_set!("99", "itemtypes.txt", "itemstatcost.txt", "weapons.txt", "armor.txt", "misc.txt");
#[cfg(feature = "embedded-data")]
static EMBEDDED_105: FileSet =
    embedded_set!("105", "itemtypes.txt", "itemstatcost.txt", "weapons.txt", "armor.txt", "misc.txt");

/// Small, fast hasher for `u32` keys (Fx hash).
#[derive(Default, Clone, Copy)]
struct FxHasher(u64);

impl Hasher for FxHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(b as u64);
        }
    }
    fn write_u32(&mut self, i: u32) {
        self.write_u64(i as u64);
    }
    fn write_u64(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

type FxMap<K, V> = HashMap<K, V, BuildHasherDefault<FxHasher>>;

/// Parsed data for a single save version.
#[derive(Debug, Clone)]
struct VersionData {
    stats: Vec<StatInfo>,
    /// Item code -> (row index in weapons+armor+misc, precomputed type info).
    items: FxMap<u32, (usize, ItemTypeInfo)>,
}

#[derive(Debug)]
enum Slot {
    Loaded(VersionData),
    #[allow(dead_code)]
    Embedded(&'static FileSet, OnceLock<VersionData>),
}

impl Slot {
    fn get(&self) -> &VersionData {
        match self {
            Slot::Loaded(data) => data,
            Slot::Embedded(files, cell) => cell.get_or_init(|| {
                parse_version(files[0], files[1], files[2], files[3], files[4]).expect("embedded game data is valid")
            }),
        }
    }
}

/// External data parsed from the original Diablo 2 .txt files
/// (itemstatcost.txt, itemtypes.txt, weapons.txt, armor.txt and misc.txt).
///
/// Data is kept per save version; a save can only be parsed if data for its exact
/// version is available.
#[derive(Debug)]
pub struct TxtData {
    versions: Vec<(u32, Slot)>,
}

impl TxtData {
    #[cfg(feature = "embedded-data")]
    /// The shared instance backed by the embedded data for versions 96, 97, 99 and 105.
    ///
    /// Each version is parsed lazily the first time it is used.
    pub fn embedded() -> &'static TxtData {
        static DEFAULT: OnceLock<TxtData> = OnceLock::new();
        DEFAULT.get_or_init(TxtData::new_embedded)
    }

    #[cfg(feature = "embedded-data")]
    /// Creates a new instance backed by the embedded data.
    pub fn new_embedded() -> TxtData {
        TxtData {
            versions: vec![
                (96, Slot::Embedded(&EMBEDDED_96, OnceLock::new())),
                (97, Slot::Embedded(&EMBEDDED_97, OnceLock::new())),
                (99, Slot::Embedded(&EMBEDDED_99, OnceLock::new())),
                (105, Slot::Embedded(&EMBEDDED_105, OnceLock::new())),
            ],
        }
    }

    /// Loads data from a directory containing version-numbered subdirectories
    /// (e.g. `96/`, `99/`), each holding the five txt files (matched case-insensitively).
    pub fn from_dir(dir: impl AsRef<Path>) -> Result<TxtData> {
        let dir = dir.as_ref();
        let mut versions = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let Some(version) = entry.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else {
                continue;
            };
            versions.push((version, Slot::Loaded(load_dir(&entry.path())?)));
        }
        if versions.is_empty() {
            return Err(Error::invalid_data(format!("no version subdirectories found in: {}", dir.display())));
        }
        versions.sort_by_key(|(v, _)| *v);
        Ok(TxtData { versions })
    }

    /// Loads data for a single version from a directory directly containing the txt files.
    pub fn from_version_dir(dir: impl AsRef<Path>, version: u32) -> Result<TxtData> {
        Ok(TxtData { versions: vec![(version, Slot::Loaded(load_dir(dir.as_ref())?))] })
    }

    /// Parses data for a single version from the contents of the five txt files.
    pub fn from_strings(
        version: u32,
        itemtypes: &str,
        itemstatcost: &str,
        weapons: &str,
        armor: &str,
        misc: &str,
    ) -> Result<TxtData> {
        let data = parse_version(itemtypes, itemstatcost, weapons, armor, misc)?;
        Ok(TxtData { versions: vec![(version, Slot::Loaded(data))] })
    }

    /// Versions with available data, in ascending order.
    pub fn versions(&self) -> Vec<u32> {
        let mut v: Vec<u32> = self.versions.iter().map(|(v, _)| *v).collect();
        v.sort_unstable();
        v
    }

    fn version(&self, save_version: u32) -> Result<&VersionData> {
        self.versions
            .iter()
            .find(|(v, _)| *v == save_version)
            .map(|(_, slot)| slot.get())
            .ok_or_else(|| Error::UnsupportedVersion { version: save_version, available: self.versions() })
    }

    /// Row index of an item code in the concatenated weapons/armor/misc tables.
    pub fn item_index(&self, code: ItemCode, save_version: u32) -> Result<Option<usize>> {
        Ok(self.version(save_version)?.items.get(&code.0).map(|(index, _)| *index))
    }
}

impl ExternalData for TxtData {
    #[inline]
    fn stat_info(&self, stat: StatId, save_version: u32) -> Result<StatInfo> {
        self.version(save_version)?.stats.get(stat.0 as usize).copied().ok_or(Error::UnknownStat(stat))
    }

    #[inline]
    fn item_info(&self, code: ItemCode, save_version: u32) -> Result<ItemTypeInfo> {
        self.version(save_version)?.items.get(&code.0).map(|(_, info)| *info).ok_or(Error::UnknownItem(code))
    }
}

fn load_dir(dir: &Path) -> Result<VersionData> {
    let mut files: Vec<(String, std::path::PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.to_ascii_lowercase().ends_with(".txt") {
            files.push((name, entry.path()));
        }
    }
    files.sort();
    let read = |name: &str| -> Result<String> {
        let (_, path) = files.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).ok_or_else(|| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("txt file not found: {}", dir.join(name).display()),
            ))
        })?;
        Ok(String::from_utf8_lossy(&std::fs::read(path)?).into_owned())
    };
    parse_version(
        &read("itemtypes.txt")?,
        &read("itemstatcost.txt")?,
        &read("weapons.txt")?,
        &read("armor.txt")?,
        &read("misc.txt")?,
    )
}

/// Splits file content into non-empty lines (a trailing `\r` is kept, matching the
/// original reader which splits on `\n` and drops empty entries).
fn lines(content: &str) -> impl Iterator<Item = &str> {
    let content = content.strip_prefix('\u{FEFF}').unwrap_or(content);
    content.split('\n').filter(|l| !l.is_empty())
}

struct Header(Vec<String>);

impl Header {
    fn parse(line: &str) -> Header {
        Header(line.trim_end_matches('\r').split('\t').map(|c| c.trim().to_owned()).collect())
    }

    /// Column index (first match, case-insensitive).
    fn col(&self, name: &str) -> Option<usize> {
        self.0.iter().position(|c| !c.is_empty() && c.eq_ignore_ascii_case(name))
    }
}

fn column<'a>(columns: &[&'a str], index: Option<usize>) -> &'a str {
    index.and_then(|i| columns.get(i)).map_or("", |c| c.trim())
}

fn parse_i32(value: &str) -> i32 {
    value.parse().unwrap_or(0)
}

fn parse_u8(value: &str) -> u8 {
    value.parse().unwrap_or(0)
}

fn parse_bool(value: &str) -> bool {
    value == "1" || value.eq_ignore_ascii_case("true")
}

fn make_code(code: &str) -> Result<u32> {
    let units: Vec<u16> = code.encode_utf16().collect();
    if units.len() > 4 {
        return Err(Error::invalid_data(format!("code must be 4 characters or less: '{code}'")));
    }
    let mut result = 0u32;
    for i in 0..4 {
        let ch = units.get(i).copied().unwrap_or(b' ' as u16) as u32;
        result |= ch << (i * 8);
    }
    Ok(result)
}

fn code_to_string(code: u32) -> String {
    let s: String = code.to_le_bytes().iter().map(|&b| b as char).collect();
    s.trim_end_matches(['\0', ' ']).to_owned()
}

fn parse_item_stat_cost(content: &str) -> Vec<StatInfo> {
    let mut lines = lines(content);
    let Some(header) = lines.next() else { return Vec::new() };
    let header = Header::parse(header);
    let col_signed = header.col("CSvSigned");
    let col_bits = header.col("CSvBits");
    let col_param = header.col("CSvParam");
    let col_save_bits = header.col("Save Bits");
    let col_save_param = header.col("Save Param Bits");
    let col_save_add = header.col("Save Add");
    let col_shift = header.col("ValShift");

    lines
        .map(|line| {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() {
                return StatInfo::default();
            }
            let columns: Vec<&str> = line.split('\t').collect();
            StatInfo {
                csv_signed: parse_bool(column(&columns, col_signed)),
                csv_bits: parse_u8(column(&columns, col_bits)),
                csv_param: parse_u8(column(&columns, col_param)),
                save_bits: parse_u8(column(&columns, col_save_bits)) as i32,
                save_param_bits: parse_i32(column(&columns, col_save_param)),
                save_add: parse_i32(column(&columns, col_save_add)),
                val_shift: parse_u8(column(&columns, col_shift)),
            }
        })
        .collect()
}

#[derive(Default)]
struct ItemTypeEntry {
    code: u32,
    equiv1: Option<usize>,
    equiv2: Option<usize>,
    var_inv_gfx: u8,
}

fn parse_item_types(content: &str) -> Result<Vec<ItemTypeEntry>> {
    let mut lines = lines(content);
    let Some(header) = lines.next() else { return Ok(Vec::new()) };
    let header = Header::parse(header);
    let col_code = header.col("Code");
    let col_equiv1 = header.col("Equiv1");
    let col_equiv2 = header.col("Equiv2");
    let col_gfx = header.col("VarInvGfx");

    let mut types = Vec::new();
    let mut equivs: Vec<(String, String)> = Vec::new();
    let mut name_to_index: HashMap<String, usize> = HashMap::new();

    for line in lines {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        let columns: Vec<&str> = line.split('\t').collect();
        let code = column(&columns, col_code);
        if code.is_empty() {
            types.push(ItemTypeEntry::default());
            equivs.push((String::new(), String::new()));
            continue;
        }
        let index = types.len();
        types.push(ItemTypeEntry {
            code: make_code(code)?,
            equiv1: None,
            equiv2: None,
            var_inv_gfx: parse_u8(column(&columns, col_gfx)),
        });
        equivs.push((column(&columns, col_equiv1).to_owned(), column(&columns, col_equiv2).to_owned()));
        name_to_index.entry(code.to_ascii_lowercase()).or_insert(index);
    }

    let resolve = |name: &str| -> Option<usize> {
        if name.is_empty() {
            None
        } else {
            name_to_index.get(&name.to_ascii_lowercase()).copied()
        }
    };
    for (entry, (e1, e2)) in types.iter_mut().zip(&equivs) {
        entry.equiv1 = resolve(e1);
        entry.equiv2 = resolve(e2);
    }
    Ok(types)
}

struct ItemEntry {
    code: u32,
    type_index: usize,
    compact_save: u8,
    stackable: u8,
    quest: u8,
    quest_diff_check: u8,
}

fn parse_items(content: &str, type_name_to_index: &HashMap<String, usize>, out: &mut Vec<ItemEntry>) -> Result<()> {
    let mut lines = lines(content);
    let Some(header) = lines.next() else { return Ok(()) };
    let header = Header::parse(header);
    let col_code = header.col("code");
    let col_type = header.col("type");
    let col_compact = header.col("compactsave");
    let col_stackable = header.col("stackable");
    let col_quest = header.col("quest");
    let col_quest_diff = header.col("questdiffcheck");

    for line in lines {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        let columns: Vec<&str> = line.split('\t').collect();
        let code = column(&columns, col_code);
        if code.is_empty() {
            continue;
        }
        let type_name = column(&columns, col_type);
        let type_index = if type_name.is_empty() {
            0
        } else {
            type_name_to_index.get(&type_name.to_ascii_lowercase()).copied().unwrap_or(0)
        };
        out.push(ItemEntry {
            code: make_code(code)?,
            type_index,
            compact_save: parse_u8(column(&columns, col_compact)),
            stackable: parse_u8(column(&columns, col_stackable)),
            quest: parse_u8(column(&columns, col_quest)),
            quest_diff_check: parse_u8(column(&columns, col_quest_diff)),
        });
    }
    Ok(())
}

/// All item type codes that are `target` or descend from it (via Equiv1/Equiv2).
fn descendants(types: &[ItemTypeEntry], children: &[Vec<usize>], target: u32) -> HashSet<u32> {
    let mut set = HashSet::new();
    let mut visited = vec![false; types.len()];
    let mut stack: Vec<usize> = (0..types.len()).filter(|&i| types[i].code == target).collect();
    while let Some(i) = stack.pop() {
        if std::mem::replace(&mut visited[i], true) {
            continue;
        }
        if types[i].code != 0 {
            set.insert(types[i].code);
        }
        stack.extend(children[i].iter().copied());
    }
    set
}

fn parse_version(itemtypes: &str, itemstatcost: &str, weapons: &str, armor: &str, misc: &str) -> Result<VersionData> {
    let types = parse_item_types(itemtypes)?;
    let stats = parse_item_stat_cost(itemstatcost);

    let mut type_name_to_index = HashMap::new();
    for (i, t) in types.iter().enumerate() {
        if t.code != 0 {
            type_name_to_index.entry(code_to_string(t.code).to_ascii_lowercase()).or_insert(i);
        }
    }

    let mut items = Vec::new();
    parse_items(weapons, &type_name_to_index, &mut items)?;
    parse_items(armor, &type_name_to_index, &mut items)?;
    parse_items(misc, &type_name_to_index, &mut items)?;

    // Type hierarchy: a type's parents are its Equiv1/Equiv2 (index 0 is never a parent).
    let mut children = vec![Vec::new(); types.len()];
    for (i, t) in types.iter().enumerate() {
        for parent in [t.equiv1, t.equiv2].into_iter().flatten() {
            if parent != 0 {
                children[parent].push(i);
            }
        }
    }
    let target = |code: &str| descendants(&types, &children, ItemCode::new(code).0);
    let armor_set = target("armo");
    let weapon_set = target("weap");
    let gold_set = target("gold");
    let charm_set = target("char");
    let body_set = target("body");
    let player_body_set = target("play");
    let scroll_set = target("scro");
    let book_set = target("book");

    let mut map: FxMap<u32, (usize, ItemTypeInfo)> = FxMap::default();
    for (index, item) in items.iter().enumerate() {
        if item.code == 0 || map.contains_key(&item.code) {
            continue;
        }
        let item_type = types.get(item.type_index);
        let type_code = item_type.map_or(0, |t| t.code);
        let is = |set: &HashSet<u32>| type_code != 0 && set.contains(&type_code);
        let info = ItemTypeInfo {
            compact_save: item.compact_save != 0,
            has_variable_gfx: item_type.is_some_and(|t| t.var_inv_gfx > 0),
            is_armor: is(&armor_set),
            is_weapon: is(&weapon_set),
            is_gold: is(&gold_set),
            is_stackable: item.stackable != 0,
            is_charm: is(&charm_set),
            is_body_part: is(&body_set),
            is_player_body_part: is(&player_body_set),
            is_scroll_or_book: is(&scroll_set) || is(&book_set),
            is_quest: item.quest != 0,
            quest_diff_check: item.quest_diff_check != 0,
        };
        map.insert(item.code, (index, info));
    }

    Ok(VersionData { stats, items: map })
}

#[cfg(all(test, feature = "embedded-data"))]
mod tests {
    use super::*;

    #[test]
    fn embedded_versions_parse() {
        let data = TxtData::embedded();
        for v in [96, 97, 99, 105] {
            let cap = data.item_info(ItemCode::new("cap"), v).unwrap();
            assert!(cap.is_armor && !cap.is_weapon);
            let axe = data.item_info(ItemCode::new("axe"), v).unwrap();
            assert!(axe.is_weapon && !axe.is_armor);
            assert!(data.item_info(ItemCode::GOLD, v).unwrap().is_gold);
            assert!(data.item_info(ItemCode::new("cm1"), v).unwrap().is_charm);
            assert!(data.stat_info(StatId::STRENGTH, v).unwrap().save_bits > 0);
        }
        assert!(matches!(data.stat_info(StatId::STRENGTH, 98), Err(Error::UnsupportedVersion { .. })));
    }
}
