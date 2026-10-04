//! Write -> read -> write round-trips of individual items and sections.

mod common;

use common::data;
use d2s::bits::{BitReader, BitWriter};
use d2s::*;

const VERSION: u32 = D2Save::CURRENT_VERSION;

fn stored() -> ItemPosition {
    ItemPosition { mode: ItemMode::Stored, store_page: StorePage::Inventory, ..Default::default() }
}

fn gold_item(amount: u32) -> Item {
    Item {
        flags: ItemFlags::COMPACT_SAVE,
        version: 96,
        position: stored(),
        code: ItemCode::GOLD,
        gold_amount: amount,
        level: 1,
        quality: ItemQuality::Normal,
        ..Default::default()
    }
}

fn compact_item(code: &str) -> Item {
    Item {
        flags: ItemFlags::COMPACT_SAVE,
        version: 96,
        position: stored(),
        code: ItemCode::new(code),
        level: 1,
        quality: ItemQuality::Normal,
        ..Default::default()
    }
}

fn armor_item(code: &str, defense: i32, max_dur: i32, dur: i32) -> Item {
    Item {
        flags: ItemFlags::IDENTIFIED,
        version: 96,
        position: stored(),
        code: ItemCode::new(code),
        seed: 0x12345678,
        level: 50,
        quality: ItemQuality::Normal,
        defense: Some(defense),
        max_durability: Some(max_dur),
        durability: Some(dur),
        ..Default::default()
    }
}

fn weapon_item(code: &str, max_dur: i32, dur: i32) -> Item {
    Item {
        flags: ItemFlags::IDENTIFIED,
        version: 96,
        position: stored(),
        code: ItemCode::new(code),
        seed: 0x87654321,
        level: 50,
        quality: ItemQuality::Normal,
        max_durability: Some(max_dur),
        durability: Some(dur),
        ..Default::default()
    }
}

fn sockets(codes: &[&str]) -> Vec<Option<Item>> {
    codes.iter().map(|c| Some(compact_item(c))).collect()
}

fn write_with(f: impl FnOnce(&mut BitWriter<'_>)) -> Vec<u8> {
    let mut buf = Vec::new();
    f(&mut BitWriter::new(&mut buf));
    buf
}

fn assert_item_round_trip(item: &Item) {
    let written = write_with(|w| item.write(w, data(), VERSION).unwrap());
    let read = Item::read(&mut BitReader::new(&written), data(), VERSION).unwrap();
    let rewritten = write_with(|w| read.write(w, data(), VERSION).unwrap());
    assert_eq!(written, rewritten, "round-trip produced different bytes");
}

#[test]
fn compact_item_gold() {
    assert_item_round_trip(&gold_item(1000));
}

#[test]
fn compact_item_large_gold() {
    assert_item_round_trip(&gold_item(100000));
}

#[test]
fn complete_item_armor() {
    assert_item_round_trip(&armor_item("cap ", 10, 20, 15));
}

#[test]
fn complete_item_weapon() {
    assert_item_round_trip(&weapon_item("axe ", 30, 25));
}

#[test]
fn complete_item_socketed() {
    let mut item = armor_item("cap ", 10, 20, 15);
    item.flags |= ItemFlags::SOCKETED;
    item.sockets = sockets(&["gcr ", "gcb "]);
    assert_item_round_trip(&item);
}

#[test]
fn complete_item_magic() {
    let mut item = armor_item("cap ", 15, 24, 24);
    item.quality = ItemQuality::Magic;
    item.flags |= ItemFlags::IDENTIFIED;
    item.quality_data = Some(QualityData::Magic { prefix: 100, suffix: 50 });
    item.stats.push(Stat::new(StatId::FIRE_RESIST, 20));
    item.stats.push(Stat::new(StatId::INCREASED_ATTACK_SPEED, 10));
    assert_item_round_trip(&item);
}

#[test]
fn complete_item_rare() {
    let mut item = armor_item("cap ", 20, 30, 30);
    item.quality = ItemQuality::Rare;
    item.flags |= ItemFlags::IDENTIFIED;
    item.quality_data = Some(QualityData::RareCraft(RareCraftData {
        name_prefix: 50,
        name_suffix: 60,
        prefixes: [100, 0, 200],
        suffixes: [50, 75, 0],
    }));
    item.stats.push(Stat::new(StatId::FIRE_RESIST, 30));
    item.stats.push(Stat::new(StatId::LIGHTNING_RESIST, 25));
    assert_item_round_trip(&item);
}

#[test]
fn complete_item_cold_damage() {
    let mut item = weapon_item("axe ", 40, 40);
    item.quality = ItemQuality::Magic;
    item.flags |= ItemFlags::IDENTIFIED;
    item.quality_data = Some(QualityData::Magic { prefix: 100, suffix: 0 });
    item.stats.push(Stat::new(StatId::COLD_MIN_DAMAGE, 5));
    item.stats.push(Stat::new(StatId::COLD_MAX_DAMAGE, 10));
    item.stats.push(Stat::new(StatId::COLD_LENGTH, 50));
    assert_item_round_trip(&item);
}

#[test]
fn complete_item_poison_damage() {
    let mut item = weapon_item("dgr ", 30, 30);
    item.quality = ItemQuality::Magic;
    item.flags |= ItemFlags::IDENTIFIED;
    item.quality_data = Some(QualityData::Magic { prefix: 150, suffix: 0 });
    item.stats.push(Stat::new(StatId::POISON_MIN_DAMAGE, 10));
    item.stats.push(Stat::new(StatId::POISON_MAX_DAMAGE, 20));
    item.stats.push(Stat::new(StatId::POISON_LENGTH, 100));
    assert_item_round_trip(&item);
}

#[test]
fn complete_item_personalized() {
    let mut item = armor_item("cap ", 25, 35, 35);
    item.quality = ItemQuality::Unique;
    item.flags |= ItemFlags::IDENTIFIED | ItemFlags::PERSONALIZED;
    item.personalized_name = "TestName".into();
    item.quality_data = Some(QualityData::SetUnique { file_index: 5 });
    assert_item_round_trip(&item);
}

#[test]
fn complete_item_runeword() {
    let mut item = weapon_item("2hs ", 50, 50);
    item.quality = ItemQuality::Normal;
    item.flags |= ItemFlags::IDENTIFIED | ItemFlags::RUNEWORD | ItemFlags::SOCKETED;
    item.runeword_id = Some(27);
    item.sockets = sockets(&["r07 ", "r09 ", "r11 ", "r14 "]);
    item.runeword_stats = Some(vec![
        Stat::new(StatId::FIRE_RESIST, 35),
        Stat::new(StatId::LIGHTNING_RESIST, 35),
        Stat::new(StatId::COLD_RESIST, 35),
        Stat::new(StatId::POISON_RESIST, 35),
    ]);
    assert_item_round_trip(&item);
}

#[test]
fn complete_item_realm_data() {
    let mut item = armor_item("cap ", 10, 20, 15);
    item.realm_data = Some(vec![0x12345678, 0xABCDEF01]);
    assert_item_round_trip(&item);
}

/// Regression: raw store page 0 was read as Null but written as 7.
#[test]
fn complete_item_store_page_null() {
    let item = Item {
        flags: ItemFlags::IDENTIFIED,
        version: 96,
        position: ItemPosition { mode: ItemMode::Stored, store_page: StorePage::Null, ..Default::default() },
        code: ItemCode::new("gcr "),
        seed: 0x12345678,
        level: 50,
        quality: ItemQuality::Normal,
        ..Default::default()
    };
    assert_item_round_trip(&item);
}

#[test]
fn complete_item_set_with_bonuses() {
    let mut item = armor_item("cap ", 30, 40, 40);
    item.quality = ItemQuality::Set;
    item.flags |= ItemFlags::IDENTIFIED;
    item.quality_data = Some(QualityData::SetUnique { file_index: 10 });
    item.set_item_mask = Some(0b00101);
    item.stats.push(Stat::new(StatId::FIRE_RESIST, 15));
    item.set_bonus_stats.push(vec![Stat::new(StatId::LIGHTNING_RESIST, 10)]);
    item.set_bonus_stats.push(vec![Stat::new(StatId::COLD_RESIST, 10)]);
    assert_item_round_trip(&item);
}

/// Deterministic xorshift PRNG for the fuzz test.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Uniform in `[lo, hi)`.
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % (hi - lo) as u64) as i64
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.range(0, items.len() as i64) as usize]
    }
}

fn random_item(rng: &mut Rng) -> Item {
    if rng.range(0, 2) == 0 {
        if rng.range(0, 2) == 0 {
            return gold_item(rng.range(1, 1_000_000) as u32);
        }
        let codes = ["gcr ", "gcb ", "gcg ", "gcy ", "gcv ", "gcw ", "r01 ", "r05 ", "r10 "];
        let code = codes[rng.range(0, codes.len() as i64) as usize];
        return compact_item(code);
    }

    let armor = ["cap ", "hlm ", "fhl ", "lea ", "hla ", "qui ", "buc ", "sml "];
    let weapons = ["axe ", "2ax ", "ssd ", "lsd ", "bsw ", "sbw ", "hbw "];
    let mut item = if rng.range(0, 2) == 0 {
        let code = *rng.pick(&armor);
        armor_item(code, rng.range(1, 100) as i32, rng.range(1, 255) as i32, rng.range(1, 255) as i32)
    } else {
        let code = *rng.pick(&weapons);
        weapon_item(code, rng.range(1, 255) as i32, rng.range(1, 255) as i32)
    };

    item.quality = *rng.pick(&[ItemQuality::Normal, ItemQuality::Magic, ItemQuality::Rare]);
    match item.quality {
        ItemQuality::Magic => {
            item.quality_data =
                Some(QualityData::Magic { prefix: rng.range(0, 500) as u16, suffix: rng.range(0, 500) as u16 })
        }
        ItemQuality::Rare => {
            item.flags |= ItemFlags::IDENTIFIED;
            item.quality_data = Some(QualityData::RareCraft(RareCraftData {
                name_prefix: rng.range(1, 200) as u8,
                name_suffix: rng.range(1, 200) as u8,
                prefixes: [rng.range(0, 500) as u16, rng.range(0, 500) as u16, 0],
                suffixes: [rng.range(0, 500) as u16, 0, 0],
            }));
        }
        _ => {}
    }

    let ids = [
        StatId::FIRE_RESIST,
        StatId::LIGHTNING_RESIST,
        StatId::COLD_RESIST,
        StatId::POISON_RESIST,
        StatId::INCREASED_ATTACK_SPEED,
        StatId::REQUIREMENT_PERCENT,
    ];
    for _ in 0..rng.range(0, 5) {
        let id = *rng.pick(&ids);
        item.stats.push(Stat::new(id, rng.range(1, 50)));
    }

    if rng.range(0, 5) == 0 {
        item.realm_data = Some(vec![rng.next() as u32 >> 1, rng.next() as u32 >> 1]);
    }
    item
}

#[test]
fn fuzz_random_items() {
    let mut rng = Rng(42);
    for i in 0..100 {
        let item = random_item(&mut rng);
        let result = std::panic::catch_unwind(|| assert_item_round_trip(&item));
        assert!(result.is_ok(), "round-trip failed for item {i}: {}", item.code);
    }
}

#[test]
fn items_section() {
    let mut socketed = armor_item("hlm ", 15, 25, 20);
    socketed.flags |= ItemFlags::SOCKETED;
    socketed.sockets = sockets(&["gcr ", "gcb "]);
    let section =
        ItemsSection(vec![gold_item(5000), armor_item("cap ", 10, 20, 15), weapon_item("axe ", 30, 25), socketed]);

    let written = write_with(|w| section.write(w, data(), VERSION).unwrap());
    let read = ItemsSection::read(&mut BitReader::new(&written), data(), VERSION).unwrap();
    let rewritten = write_with(|w| read.write(w, data(), VERSION).unwrap());
    assert_eq!(written, rewritten);
}

fn assert_iron_golem_round_trip(section: &IronGolemSection) {
    let written = write_with(|w| section.write(w, data(), VERSION).unwrap());
    let read = IronGolemSection::read(&mut BitReader::new(&written), data(), VERSION).unwrap();
    if let Some(original) = &section.golem_item {
        let read_item = read.golem_item.as_ref().expect("golem item");
        assert_eq!(original.sockets.len(), read_item.sockets.len());
        for (o, r) in original.sockets.iter().zip(&read_item.sockets) {
            if let Some(o) = o {
                assert_eq!(o.code, r.as_ref().expect("socketed item").code);
            }
        }
    }
    let rewritten = write_with(|w| read.write(w, data(), VERSION).unwrap());
    assert_eq!(written, rewritten);
}

/// Regression: socketed items in the iron golem section were not read.
#[test]
fn iron_golem_section_socketed_item() {
    let mut golem_item = weapon_item("2hs ", 50, 50);
    golem_item.flags |= ItemFlags::SOCKETED;
    golem_item.sockets = sockets(&["r08 ", "r03 ", "r07 ", "r12 "]);
    assert_iron_golem_round_trip(&IronGolemSection { golem_item: Some(golem_item) });
}

#[test]
fn iron_golem_section_no_golem() {
    assert_iron_golem_round_trip(&IronGolemSection { golem_item: None });
}

fn assert_corpse_round_trip(section: &CorpseSection) {
    let written = write_with(|w| section.write(w, data(), VERSION).unwrap());
    let read = CorpseSection::read(&mut BitReader::new(&written), data(), VERSION).unwrap();
    assert_eq!(section.len(), read.len());
    for (oc, rc) in section.iter().zip(read.iter()) {
        assert_eq!(oc.items.len(), rc.items.len());
        for (oi, ri) in oc.items.iter().zip(rc.items.iter()) {
            assert_eq!(oi.sockets.len(), ri.sockets.len());
            for (o, r) in oi.sockets.iter().zip(&ri.sockets) {
                if let Some(o) = o {
                    assert_eq!(o.code, r.as_ref().expect("socketed item").code);
                }
            }
        }
    }
    let rewritten = write_with(|w| read.write(w, data(), VERSION).unwrap());
    assert_eq!(written, rewritten);
}

#[test]
fn corpse_section_socketed_item() {
    let mut armor = armor_item("plt ", 100, 60, 55);
    armor.flags |= ItemFlags::SOCKETED;
    armor.sockets = sockets(&["gcr ", "gcb "]);
    let corpse = Corpse { unused: 1, x: 5000, y: 5000, items: ItemsSection(vec![armor, weapon_item("axe ", 30, 25)]) };
    assert_corpse_round_trip(&CorpseSection(vec![corpse]));
}

#[test]
fn corpse_section_empty() {
    assert_corpse_round_trip(&CorpseSection::default());
}
