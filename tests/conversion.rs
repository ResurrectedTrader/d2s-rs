//! Conversion between 1.14 (v96) and D2R (v97+) formats, compared against saves
//! of the same character written by the game.

mod common;

use common::{compare_sections, normalized, pair_items, resource, serialize_item};
use d2s::*;

struct Conversion {
    expected_bytes: Vec<u8>,
    expected: D2Save,
    /// Source save after conversion (mutated in place).
    source: D2Save,
    converted_bytes: Vec<u8>,
}

fn convert(source: &str, expected: &str) -> Conversion {
    let expected_bytes = resource(expected);
    let expected = D2Save::from_bytes(&expected_bytes).unwrap();
    let mut source = D2Save::from_bytes(&resource(source)).unwrap();
    // Align the save time so sections can be compared exactly.
    source.character.last_save_time = expected.character.last_save_time;
    source.convert_to(expected.version);
    let converted_bytes = source.to_bytes().unwrap();
    Conversion { expected_bytes, expected, source, converted_bytes }
}

/// Item-by-item binary comparison, matching items by position and code.
fn compare_items_binary(expected: &ItemsSection, converted: &ItemsSection, version: u32, ignore_body: bool) -> usize {
    let (pairs, mut diffs) = pair_items(expected, converted);
    for (e, c) in pairs {
        let eb = serialize_item(&normalized(e, ignore_body), version);
        let cb = serialize_item(&normalized(c, ignore_body), version);
        if eb != cb {
            println!("  DIFFERENT: {} - {}", common::position_key(e), e.code);
            diffs += 1;
        }
    }
    diffs
}

/// Clears what the property comparison of items does not cover.
fn comparable_item(item: &Item, ignore_body: bool) -> Item {
    let mut item = normalized(item, ignore_body);
    let pos = item.position;
    if !pos.is_on_ground() {
        item.position.ground_x = 0;
        item.position.ground_y = 0;
    }
    if pos.mode != ItemMode::Stored {
        item.position.store_page = StorePage::Inventory;
        item.position.inv_x = 0;
        item.position.inv_y = 0;
    }
    let sort = |stats: &mut Vec<Stat>| stats.sort_by_key(|s| (s.id, s.layer, s.value));
    sort(&mut item.stats);
    item.set_bonus_stats.iter_mut().for_each(sort);
    if let Some(stats) = &mut item.runeword_stats {
        sort(stats);
    }
    // Socketed items are compared by code, level and stats only.
    for socket in item.sockets.iter_mut().flatten() {
        let mut stats = std::mem::take(&mut socket.stats);
        sort(&mut stats);
        *socket = Item { code: socket.code, level: socket.level, stats, ..Default::default() };
    }
    item.chronicle_id = None;
    item.chronicle_timestamp = None;
    item.chronicle_recipients = None;
    item.advanced_stash_stack_size = None;
    item
}

fn compare_items_by_properties(expected: &ItemsSection, converted: &ItemsSection, ignore_body: bool) -> usize {
    let (pairs, mut diffs) = pair_items(expected, converted);
    for (e, c) in pairs {
        let e = comparable_item(e, ignore_body);
        let c = comparable_item(c, ignore_body);
        if e != c {
            println!(
                "  DIFFERENT: {} - {}\n    expected:  {e:?}\n    converted: {c:?}",
                common::position_key(&e),
                e.code
            );
            diffs += 1;
        }
    }
    diffs
}

/// Compares everything except the checksum and item lists (which are compared by position).
fn compare_models(expected: &D2Save, converted: &D2Save) -> usize {
    let strip = |save: &D2Save| {
        let mut save = save.clone();
        save.checksum = 0;
        save.items.clear();
        save.merc_items = None;
        for corpse in &mut save.corpses {
            corpse.items.clear();
        }
        save
    };
    let (e, c) = (strip(expected), strip(converted));
    if e == c {
        0
    } else {
        println!("  MODEL DIFF\n    expected:  {e:#?}\n    converted: {c:#?}");
        1
    }
}

#[test]
fn convert_114_to_d2r_binary() {
    let conv = convert("96/Soska.d2s", "99/Soska.d2s");
    let v = conv.expected.version;
    let sections = compare_sections(&conv.expected_bytes, v, &conv.converted_bytes, v);
    let items = compare_items_binary(&conv.expected.items, &conv.source.items, v, false);
    assert_eq!(sections + items, 0);
}

#[test]
fn convert_d2r_to_114_binary() {
    let conv = convert("99/Soska.d2s", "96/Soska.d2s");
    let v = conv.expected.version;
    let sections = compare_sections(&conv.expected_bytes, v, &conv.converted_bytes, v);
    // D2R does not preserve body locations of stored items.
    let items = compare_items_binary(&conv.expected.items, &conv.source.items, v, true);
    assert_eq!(sections + items, 0);
}

#[test]
fn convert_114_to_d2r_model() {
    let conv = convert("96/Soska.d2s", "99/Soska.d2s");
    let converted = D2Save::from_bytes(&conv.converted_bytes).unwrap();
    let model = compare_models(&conv.expected, &converted);
    let items = compare_items_by_properties(&conv.expected.items, &converted.items, false);
    assert_eq!(model + items, 0);
}

#[test]
fn convert_d2r_to_114_model() {
    let conv = convert("99/Soska.d2s", "96/Soska.d2s");
    let converted = D2Save::from_bytes(&conv.converted_bytes).unwrap();
    let model = compare_models(&conv.expected, &converted);
    let items = compare_items_by_properties(&conv.expected.items, &converted.items, true);
    assert_eq!(model + items, 0);
}

#[test]
fn convert_to_every_version_and_back() {
    for path in ["96/Soska.d2s", "97/Druid.d2s", "99/Roka.d2s", "105/Soska.d2s", "105/ChaosSC.d2s"] {
        let bytes = resource(path);
        let original = D2Save::from_bytes(&bytes).unwrap();
        for target in [96, 97, 99, 105] {
            let mut save = original.clone();
            save.convert_to(target);
            let converted = save.to_bytes().unwrap();
            let mut reread = D2Save::from_bytes(&converted).unwrap();
            assert_eq!(reread.version, target, "{path} -> {target}");
            assert_eq!(reread.items.len(), original.items.len(), "{path} -> {target}");
            assert_eq!(reread.to_bytes().unwrap(), converted, "{path} -> {target} is not stable");
            reread.convert_to(original.version);
            let back = D2Save::from_bytes(&reread.to_bytes().unwrap()).unwrap();
            // v97 keeps the name in the character section, but conversion to 1.14 (like the
            // reference implementation) takes it from the preview, so it is lost there.
            if !(original.version == 97 && target == 96) {
                assert_eq!(back.name(), original.name(), "{path} -> {target} -> back");
            }
            assert_eq!(back.is_expansion(), original.is_expansion(), "{path} -> {target} -> back");
        }
    }
}
