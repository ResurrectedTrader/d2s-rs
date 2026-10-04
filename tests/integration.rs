//! Round-trips and conversions of real save files.

mod common;

use common::{assert_save_round_trip, data, resource, resource_path};
use d2s::bits::{BitReader, BitWriter};
use d2s::*;

/// Reads Roka.d2s section by section and checks each section writes back identically.
#[test]
fn round_trip_known_save_per_section() {
    let bytes = resource("99/Roka.d2s");
    let ext = data();
    let mut r = BitReader::new(&bytes);
    let mut read_positions = Vec::new();

    assert_eq!(r.read_u32().unwrap(), D2Save::MAGIC);
    let version = r.read_u32().unwrap();
    r.read_u32().unwrap();
    r.read_u32().unwrap();
    read_positions.push(("Header", r.byte_position()));
    let character = Character::read(&mut r, version).unwrap();
    read_positions.push(("Character", r.byte_position()));
    let quests = QuestSection::read(&mut r).unwrap();
    read_positions.push(("Quests", r.byte_position()));
    let waypoints = WaypointSection::read(&mut r).unwrap();
    read_positions.push(("Waypoints", r.byte_position()));
    let intro = PlayerIntroSection::read(&mut r).unwrap();
    read_positions.push(("PlayerIntro", r.byte_position()));
    let stats = PlayerStats::read(&mut r, ext, version).unwrap();
    read_positions.push(("Stats", r.byte_position()));
    let skills = SkillsSection::read(&mut r, character.class, character.num_skills as usize).unwrap();
    read_positions.push(("Skills", r.byte_position()));
    let items = ItemsSection::read(&mut r, ext, version).unwrap();
    read_positions.push(("Items", r.byte_position()));
    let corpses = CorpseSection::read(&mut r, ext, version).unwrap();
    read_positions.push(("Corpses", r.byte_position()));
    let expansion = character.flags.contains(CharacterFlags::EXPANSION);
    let mut merc = None;
    let mut golem = None;
    if expansion {
        merc = Some(MercItemsSection::read(&mut r, ext, character.merc.has_merc(), version).unwrap());
        read_positions.push(("MercItems", r.byte_position()));
        golem = Some(IronGolemSection::read(&mut r, ext, version).unwrap());
        read_positions.push(("IronGolem", r.byte_position()));
    }

    let mut buf = Vec::new();
    let mut w = BitWriter::new(&mut buf);
    let mut write_positions = Vec::new();
    w.write_u32(D2Save::MAGIC).unwrap();
    w.write_u32(version).unwrap();
    w.write_u32(0).unwrap();
    w.write_u32(0).unwrap();
    write_positions.push(w.bytes_written());
    character.write(&mut w, version).unwrap();
    write_positions.push(w.bytes_written());
    quests.write(&mut w).unwrap();
    write_positions.push(w.bytes_written());
    waypoints.write(&mut w).unwrap();
    write_positions.push(w.bytes_written());
    intro.write(&mut w).unwrap();
    write_positions.push(w.bytes_written());
    stats.write(&mut w, ext, version).unwrap();
    write_positions.push(w.bytes_written());
    skills.write(&mut w).unwrap();
    write_positions.push(w.bytes_written());
    items.write(&mut w, ext, version).unwrap();
    write_positions.push(w.bytes_written());
    corpses.write(&mut w, ext, version).unwrap();
    write_positions.push(w.bytes_written());
    if expansion {
        merc.unwrap_or_default().write(&mut w, ext, character.merc.has_merc(), version).unwrap();
        write_positions.push(w.bytes_written());
        golem.unwrap_or_default().write(&mut w, ext, version).unwrap();
        write_positions.push(w.bytes_written());
    }
    let len = w.bytes_written();
    w.patch_u32(8, len as u32);
    checksum::update(&mut buf).unwrap();

    let mut all_match = true;
    let (mut prev_r, mut prev_w) = (0, 0);
    for ((name, end_r), end_w) in read_positions.iter().zip(&write_positions) {
        let ok = bytes[prev_r..*end_r] == buf[prev_w..*end_w];
        println!(
            "{name}: {} (read {} bytes, wrote {} bytes)",
            if ok { "OK" } else { "MISMATCH" },
            end_r - prev_r,
            end_w - prev_w
        );
        all_match &= ok;
        prev_r = *end_r;
        prev_w = *end_w;
    }
    assert!(all_match, "round-trip produced different bytes in one or more sections");
    assert_eq!(bytes.len(), len);
}

fn assert_stash_round_trip(path: &str) -> D2StashSave {
    let bytes = resource(path);
    let stash = D2StashSave::from_bytes(&bytes).unwrap();
    assert!(!stash.is_empty());
    let written = stash.to_bytes().unwrap();
    assert_eq!(written.len(), bytes.len());
    assert!(written == bytes, "{path} round-trip produced different bytes");
    stash
}

#[test]
fn round_trip_stash_99() {
    assert_stash_round_trip("99/SharedStashSoftCoreV2.d2i");
}

#[test]
fn verify_checksum() {
    assert!(D2Save::verify_checksum(&resource("99/Roka.d2s")));
}

#[test]
fn round_trip_modded_save() {
    let ext = TxtData::from_dir(resource_path("Modded/Txt")).unwrap();
    let bytes = resource("Modded/Watcher.d2s");
    let save = D2Save::from_bytes_with(&bytes, &ext).unwrap();
    let written = save.to_bytes_with(&ext).unwrap();
    assert_eq!(written.len(), bytes.len());
    assert!(written == bytes, "modded save round-trip produced different bytes");
}

#[test]
fn modded_save_from_single_version_dir() {
    let bytes = resource("Modded/Watcher.d2s");
    let ext = TxtData::from_version_dir(resource_path("Modded/Txt/99"), 99).unwrap();
    assert!(D2Save::from_bytes_with(&bytes, &ext).is_ok());
}

#[test]
fn round_trip_96_save() {
    assert_save_round_trip(&resource("96/Soska.d2s"));
}

#[test]
fn round_trip_97_saves() {
    for name in ["Amazon", "Assassin", "Barbarian", "DannyIsGreat", "Druid", "Necromancer", "Paladin", "Sorceress"] {
        assert_save_round_trip(&resource(&format!("97/{name}.d2s")));
    }
}

#[test]
fn round_trip_99_saves() {
    for name in ["Roka", "Soska"] {
        assert_save_round_trip(&resource(&format!("99/{name}.d2s")));
    }
}

#[test]
fn round_trip_105_saves() {
    for name in ["Roka", "Soska", "ChaosSC"] {
        let save = assert_save_round_trip(&resource(&format!("105/{name}.d2s")));
        assert_eq!(save.version, 105);
        assert!(!save.character.preview.name.is_empty(), "v105 name should be in preview");
    }
}

#[test]
fn v105_demon_section_chaos_sc() {
    let save = D2Save::from_bytes(&resource("105/ChaosSC.d2s")).unwrap();
    assert_eq!(save.character.preview.game_version, GameVersion::ReignOfTheWarlock);
    assert_eq!(save.character.class, CharacterClass::Warlock);
    let demon = save.demon.as_ref().expect("demon section");
    assert!(demon.has_demon);
    let data = demon.demon.as_ref().expect("demon data");
    assert!(!data.stats.is_empty(), "demon should have stats");
}

#[test]
fn round_trip_105_stash() {
    assert_stash_round_trip("105/SharedStashSoftCoreV2.d2i");
}

#[test]
fn round_trip_105_modern_stash() {
    let stash = assert_stash_round_trip("105/ModernSharedStashSoftCoreV2.d2i");
    assert_eq!(stash.len(), 11);
    assert_eq!(stash[0].tab_type, StashTabType::Normal);
    assert_eq!(stash[9].tab_type, StashTabType::AdvancedStash);
    assert_eq!(stash[10].tab_type, StashTabType::Chronicle);

    let chronicle = stash[10].chronicle.as_ref().expect("chronicle");
    assert_eq!(ChronicleSection::MAGIC, 0xC0EAEDC0);
    assert_eq!(chronicle.version, 1);
    assert!(chronicle.set_entries.is_empty());
    assert!(chronicle.unique_entries.is_empty());
    assert!(chronicle.runeword_entries.is_empty());
    // 64 bytes of stale buffer data written by the game.
    assert_eq!(chronicle.trailing_data.len(), 64);
}

#[test]
fn convert_99_to_105_round_trip() {
    let mut original = D2Save::from_bytes(&resource("99/Roka.d2s")).unwrap();
    let name = original.character.preview.name.clone();

    original.convert_to(105);
    let bytes_105 = original.to_bytes().unwrap();
    let mut save_105 = D2Save::from_bytes(&bytes_105).unwrap();
    assert_eq!(save_105.version, 105);
    assert_eq!(save_105.character.preview.name, name);

    save_105.convert_to(99);
    let restored = D2Save::from_bytes(&save_105.to_bytes().unwrap()).unwrap();
    assert_eq!(restored.version, 99);
    assert_eq!(restored.character.preview.name, name);
    assert_eq!(restored.character.level, save_105.character.level);
    assert_eq!(restored.character.class, save_105.character.class);
}

/// v104+ moves the expansion flag into the game version; expansion sections must survive.
#[test]
fn convert_99_to_105_preserves_expansion_sections() {
    let mut original = D2Save::from_bytes(&resource("99/Roka.d2s")).unwrap();
    assert!(original.character.flags.contains(CharacterFlags::EXPANSION));
    assert!(original.merc_items.is_some());
    assert!(original.iron_golem.is_some());

    original.convert_to(105);
    let mut save_105 = D2Save::from_bytes(&original.to_bytes().unwrap()).unwrap();
    assert_eq!(save_105.character.preview.game_version, GameVersion::Expansion);
    assert!(save_105.merc_items.is_some());
    assert!(save_105.iron_golem.is_some());

    save_105.convert_to(99);
    let restored = D2Save::from_bytes(&save_105.to_bytes().unwrap()).unwrap();
    assert!(restored.character.flags.contains(CharacterFlags::EXPANSION));
    assert!(restored.merc_items.is_some());
    assert!(restored.iron_golem.is_some());
}

#[test]
fn convert_stash_99_to_105_round_trip() {
    let original = D2StashSave::from_bytes(&resource("99/SharedStashSoftCoreV2.d2i")).unwrap();

    let mut stash = original.clone();
    stash.convert_to(105);
    let mut stash_105 = D2StashSave::from_bytes(&stash.to_bytes().unwrap()).unwrap();
    assert_eq!(stash_105.len(), original.len());
    for tab in &stash_105 {
        assert_eq!(tab.save_version, 105);
        assert_eq!(tab.stash_format, 2);
    }

    stash_105.convert_to(99);
    let restored = D2StashSave::from_bytes(&stash_105.to_bytes().unwrap()).unwrap();
    assert_eq!(restored.len(), original.len());
    for (o, r) in original.iter().zip(restored.iter()) {
        assert_eq!(o.items.len(), r.items.len());
        assert_eq!(r.stash_format, 1);
    }
}

#[test]
fn stash_tab_reads_single_tab() {
    let bytes = resource("105/ModernSharedStashSoftCoreV2.d2i");
    let tab = D2StashTab::from_bytes(&bytes).unwrap();
    let size = u16::from_le_bytes([bytes[16], bytes[17]]) as usize;
    assert_eq!(tab.to_bytes().unwrap(), bytes[..size]);
}

#[test]
fn trailing_data_is_preserved() {
    let mut bytes = resource("99/Roka.d2s");
    bytes.extend_from_slice(b"MODDATA");
    checksum::update(&mut bytes).unwrap();
    let len = bytes.len() as u32;
    bytes[8..12].copy_from_slice(&len.to_le_bytes());
    checksum::update(&mut bytes).unwrap();

    let save = D2Save::from_bytes(&bytes).unwrap();
    assert_eq!(save.trailing_data, b"MODDATA");
    assert_eq!(save.to_bytes().unwrap(), bytes);
}

#[test]
fn invalid_magic_is_rejected() {
    let mut bytes = resource("99/Roka.d2s");
    bytes[0] = 0;
    assert!(matches!(D2Save::from_bytes(&bytes), Err(Error::InvalidMagic { .. })));
}

#[test]
fn truncated_save_is_rejected() {
    let bytes = resource("99/Roka.d2s");
    for len in [0, 10, 100, 765, bytes.len() / 2, bytes.len() - 1] {
        let result = D2Save::from_bytes(&bytes[..len]);
        // Truncating inside the trailing area is fine only at section boundaries; never panics.
        if len < bytes.len() / 2 {
            assert!(result.is_err(), "truncated to {len} bytes should fail");
        }
    }
}

#[test]
fn write_to_reuses_buffer() {
    let bytes = resource("99/Roka.d2s");
    let save = D2Save::from_bytes(&bytes).unwrap();
    let mut buf = b"prefix".to_vec();
    let n = save.write_to(&mut buf, data()).unwrap();
    assert_eq!(n, bytes.len());
    assert_eq!(&buf[..6], b"prefix");
    assert_eq!(&buf[6..], &bytes[..]);
}
