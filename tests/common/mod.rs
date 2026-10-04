#![allow(dead_code)]

use std::path::PathBuf;

use d2s::bits::{BitReader, BitWriter};
use d2s::{BodyLocation, D2Save, Item, ItemMode, ItemsSection, TxtData};

pub fn resource_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("resources").join(path)
}

pub fn resource(path: &str) -> Vec<u8> {
    std::fs::read(resource_path(path)).unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
}

pub fn data() -> &'static TxtData {
    TxtData::embedded()
}

/// Byte offsets (end positions) of each section of a save file.
pub fn section_positions(data: &[u8], version: u32) -> Vec<(&'static str, usize)> {
    use d2s::*;
    let ext = self::data();
    let mut positions = Vec::new();
    let mut r = BitReader::new(data);
    for _ in 0..4 {
        r.read_u32().unwrap();
    }
    positions.push(("Header", r.byte_position()));
    let character = Character::read(&mut r, version).unwrap();
    positions.push(("Character", r.byte_position()));
    QuestSection::read(&mut r).unwrap();
    positions.push(("Quests", r.byte_position()));
    WaypointSection::read(&mut r).unwrap();
    positions.push(("Waypoints", r.byte_position()));
    PlayerIntroSection::read(&mut r).unwrap();
    positions.push(("PlayerIntro", r.byte_position()));
    PlayerStats::read(&mut r, ext, version).unwrap();
    positions.push(("Stats", r.byte_position()));
    SkillsSection::read(&mut r, character.class, character.num_skills as usize).unwrap();
    positions.push(("Skills", r.byte_position()));
    ItemsSection::read(&mut r, ext, version).unwrap();
    positions.push(("Items", r.byte_position()));
    CorpseSection::read(&mut r, ext, version).unwrap();
    positions.push(("Corpses", r.byte_position()));
    if character.flags.contains(CharacterFlags::EXPANSION) {
        MercItemsSection::read(&mut r, ext, character.merc.has_merc(), version).unwrap();
        positions.push(("MercItems", r.byte_position()));
        IronGolemSection::read(&mut r, ext, version).unwrap();
        positions.push(("IronGolem", r.byte_position()));
    }
    positions
}

pub fn serialize_item(item: &Item, version: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    item.write(&mut BitWriter::new(&mut buf), data(), version).unwrap();
    buf
}

pub fn position_key(item: &Item) -> String {
    let pos = &item.position;
    if pos.is_on_ground() {
        format!("Ground({},{})", pos.ground_x, pos.ground_y)
    } else if pos.mode == ItemMode::Equipped {
        format!("Equipped/{:?}", pos.body_location)
    } else {
        format!("{:?}/{:?}({},{})", pos.mode, pos.store_page, pos.inv_x, pos.inv_y)
    }
}

/// Pairs items by position key and item code (in order of appearance).
pub fn pair_items<'a>(expected: &'a ItemsSection, actual: &'a ItemsSection) -> (Vec<(&'a Item, &'a Item)>, usize) {
    use std::collections::BTreeMap;
    type Group<'a> = (Vec<&'a Item>, Vec<&'a Item>);
    let mut groups: BTreeMap<(String, String), Group<'_>> = BTreeMap::new();
    for item in expected {
        groups.entry((position_key(item), item.code_string())).or_default().0.push(item);
    }
    for item in actual {
        groups.entry((position_key(item), item.code_string())).or_default().1.push(item);
    }
    let mut pairs = Vec::new();
    let mut unmatched = 0;
    for ((pos, code), (exp, act)) in groups {
        let n = exp.len().min(act.len());
        for i in 0..n {
            pairs.push((exp[i], act[i]));
        }
        for _ in n..exp.len() {
            println!("  ONLY IN EXPECTED: {pos} - {code}");
            unmatched += 1;
        }
        for _ in n..act.len() {
            println!("  ONLY IN CONVERTED: {pos} - {code}");
            unmatched += 1;
        }
    }
    (pairs, unmatched)
}

/// Clears body location of a stored item copy (D2R does not preserve it).
pub fn normalized(item: &Item, ignore_stored_body_location: bool) -> Item {
    let mut item = item.clone();
    if ignore_stored_body_location && item.position.mode != ItemMode::Equipped {
        item.position.body_location = BodyLocation::None;
    }
    item
}

/// Section-by-section comparison; returns the number of differing sections
/// (ignoring Header and Items, which are compared separately).
pub fn compare_sections(data1: &[u8], v1: u32, data2: &[u8], v2: u32) -> usize {
    let p1 = section_positions(data1, v1);
    let p2 = section_positions(data2, v2);
    let (mut prev1, mut prev2, mut diffs) = (0, 0, 0);
    for ((name, end1), (_, end2)) in p1.iter().zip(&p2) {
        let a = &data1[prev1..*end1];
        let b = &data2[prev2..*end2];
        let ok = a == b;
        println!("{name:<15} {:>8} {:>8} {}", a.len(), b.len(), if ok { "OK" } else { "DIFF" });
        if !ok && *name != "Items" && *name != "Header" {
            diffs += 1;
        }
        prev1 = *end1;
        prev2 = *end2;
    }
    diffs
}

/// Parses `data`, writes it back and asserts the output is byte-identical.
pub fn assert_save_round_trip(data: &[u8]) -> D2Save {
    let save = D2Save::from_bytes(data).unwrap();
    let written = save.to_bytes().unwrap();
    assert_eq!(written.len(), data.len());
    assert!(written == data, "round-trip produced different bytes");
    save
}
