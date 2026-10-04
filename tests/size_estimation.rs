//! `estimate_size` must be large enough for worst-case saves.

use d2s::*;

/// 16 mathematical alphanumeric symbols, 4 UTF-8 bytes each: 64 bytes, the maximum.
const LONG_NAME: &str = "𝐀𝐁𝐂𝐃𝐄𝐅𝐆𝐇𝐈𝐉𝐊𝐋𝐌𝐍𝐎𝐏";

fn base_save() -> D2Save {
    let mut stats = PlayerStats::default();
    stats.set(StatId::STRENGTH, 100);
    stats.set(StatId::DEXTERITY, 100);
    stats.set(StatId::VITALITY, 100);
    stats.set(StatId::ENERGY, 100);
    stats.set(StatId::LEVEL, 99);
    stats.set(StatId::EXPERIENCE, 3520485254);
    stats.set(StatId::GOLD, 0);
    stats.set(StatId::STASH_GOLD, 2500000);

    D2Save {
        version: 96,
        character: Character {
            name: "TestChar".into(),
            class: CharacterClass::Amazon,
            level: 99,
            flags: CharacterFlags::EXPANSION,
            num_skills: 30,
            ..Default::default()
        },
        stats,
        skills: SkillsSection { class: CharacterClass::Amazon, levels: vec![0; 30] },
        merc_items: Some(MercItemsSection::default()),
        iron_golem: Some(IronGolemSection::default()),
        ..Default::default()
    }
}

fn eight_stats(attr: i64, fire: (i64, i64), light_max: i64, cold: (i64, i64, i64), poison: (i64, i64)) -> Vec<Stat> {
    vec![
        Stat::new(StatId::STRENGTH, attr),
        Stat::new(StatId::DEXTERITY, attr),
        Stat::new(StatId::VITALITY, attr),
        Stat::new(StatId::ENERGY, attr),
        Stat::new(StatId::FIRE_RESIST, 30),
        Stat::new(StatId::LIGHTNING_RESIST, 30),
        Stat::new(StatId::COLD_RESIST, 30),
        Stat::new(StatId::POISON_RESIST, 30),
        Stat::new(StatId::FIRE_MIN_DAMAGE, fire.0),
        Stat::new(StatId::FIRE_MAX_DAMAGE, fire.1),
        Stat::new(StatId::LIGHTNING_MIN_DAMAGE, 1),
        Stat::new(StatId::LIGHTNING_MAX_DAMAGE, light_max),
        Stat::new(StatId::COLD_MIN_DAMAGE, cold.0),
        Stat::new(StatId::COLD_MAX_DAMAGE, cold.1),
        Stat::new(StatId::COLD_LENGTH, cold.2),
        Stat::new(StatId::POISON_MIN_DAMAGE, poison.0),
        Stat::new(StatId::POISON_MAX_DAMAGE, poison.0),
        Stat::new(StatId::POISON_LENGTH, poison.1),
    ]
}

fn rare_data() -> Option<QualityData> {
    Some(QualityData::RareCraft(RareCraftData {
        name_prefix: 50,
        name_suffix: 60,
        prefixes: [100, 150, 200],
        suffixes: [50, 75, 100],
    }))
}

fn rare_ring(page: StorePage, x: u8, y: u8) -> Item {
    Item {
        flags: ItemFlags::IDENTIFIED | ItemFlags::PERSONALIZED,
        version: 96,
        position: ItemPosition { mode: ItemMode::Stored, store_page: page, inv_x: x, inv_y: y, ..Default::default() },
        code: ItemCode::new("rin "),
        seed: 0x12345678,
        level: 99,
        quality: ItemQuality::Rare,
        personalized_name: LONG_NAME.into(),
        quality_data: rare_data(),
        stats: eight_stats(20, (10, 20), 30, (10, 20, 100), (50, 150)),
        ..Default::default()
    }
}

fn rare_jewel() -> Item {
    Item {
        flags: ItemFlags::IDENTIFIED,
        version: 99,
        position: ItemPosition { mode: ItemMode::Stored, store_page: StorePage::Null, ..Default::default() },
        code: ItemCode::new("jew "),
        seed: 0xDEADBEEF,
        level: 99,
        quality: ItemQuality::Rare,
        quality_data: rare_data(),
        stats: eight_stats(15, (5, 10), 15, (5, 10, 50), (25, 75)),
        ..Default::default()
    }
}

fn socketed_wand(page: StorePage, x: u8, y: u8) -> Item {
    Item {
        flags: ItemFlags::IDENTIFIED | ItemFlags::SOCKETED | ItemFlags::PERSONALIZED,
        version: 99,
        position: ItemPosition { mode: ItemMode::Stored, store_page: page, inv_x: x, inv_y: y, ..Default::default() },
        code: ItemCode::new("wnd "),
        seed: 0x87654321,
        level: 99,
        quality: ItemQuality::Rare,
        quality_data: rare_data(),
        personalized_name: LONG_NAME.into(),
        max_durability: Some(40),
        durability: Some(40),
        stats: eight_stats(20, (10, 20), 30, (10, 20, 100), (50, 150)),
        sockets: vec![Some(rare_jewel()), Some(rare_jewel())],
        ..Default::default()
    }
}

fn assert_estimate_sufficient(save: &D2Save) {
    let estimate = save.estimate_size();
    let actual = save.to_bytes().unwrap().len();
    println!("estimated {estimate}, actual {actual}");
    assert!(actual <= estimate, "estimated size ({estimate}) was smaller than actual size ({actual})");
}

fn fill(save: &mut D2Save, page: StorePage, width: u8, height: u8) {
    for y in 0..height {
        for x in 0..width {
            save.items.push(rare_ring(page, x, y));
        }
    }
}

#[test]
fn maximum_rare_personalized_rings() {
    let mut save = base_save();
    fill(&mut save, StorePage::Stash, 10, 10);
    fill(&mut save, StorePage::Inventory, 10, 4);
    assert_estimate_sufficient(&save);
}

#[test]
fn socketed_items() {
    let mut save = base_save();
    save.version = 99;
    for i in 0..50u8 {
        save.items.push(socketed_wand(StorePage::Stash, i % 10, i / 10 * 2));
    }
    assert_estimate_sufficient(&save);
}

#[test]
fn extreme_stress() {
    let mut save = base_save();
    fill(&mut save, StorePage::Stash, 10, 10);
    fill(&mut save, StorePage::Inventory, 10, 4);
    fill(&mut save, StorePage::Cube, 3, 4);
    assert_estimate_sufficient(&save);
}
