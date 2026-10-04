//! Overlay views must agree with the parsed model.

mod common;

use common::resource;
use d2s::overlay::*;
use d2s::*;

const V103_SAVES: [&str; 11] = [
    "96/Soska.d2s",
    "97/Amazon.d2s",
    "97/Assassin.d2s",
    "97/Barbarian.d2s",
    "97/DannyIsGreat.d2s",
    "97/Druid.d2s",
    "97/Necromancer.d2s",
    "97/Paladin.d2s",
    "97/Sorceress.d2s",
    "99/Roka.d2s",
    "99/Soska.d2s",
];

const V104_SAVES: [&str; 3] = ["105/Soska.d2s", "105/Roka.d2s", "105/ChaosSC.d2s"];

fn assert_preview_item(expected: &PreviewItem, actual: PreviewItemLayout<&[u8]>) {
    assert_eq!(expected.code, actual.code());
    assert_eq!(expected.transform, actual.transform());
    assert_eq!(expected.quality, actual.quality());
    assert_eq!(expected.file_index, actual.file_index());
    assert_eq!(expected.flags, actual.flags());
}

fn assert_merc(expected: &MercData, actual: MercDataLayout<&[u8]>) {
    assert_eq!(expected.flags, actual.flags());
    assert_eq!(expected.seed, actual.seed());
    assert_eq!(expected.name_index, actual.name_index());
    assert_eq!(expected.hireling_id, actual.hireling_id());
    assert_eq!(expected.experience, actual.experience());
    assert_eq!(expected.is_dead(), actual.is_dead());
    assert_eq!(expected.has_merc(), actual.has_merc());
}

fn assert_waypoints(parsed: &D2Save, waypoints: WaypointSectionLayout<&[u8]>) {
    assert_eq!(parsed.waypoints.version, waypoints.version());
    assert_eq!(parsed.waypoints.section_size, waypoints.section_size());
    for wp in 0..39 {
        assert_eq!(parsed.waypoints.normal.is_active(wp), waypoints.normal().is_active(wp));
        assert_eq!(parsed.waypoints.nightmare.is_active(wp), waypoints.nightmare().is_active(wp));
        assert_eq!(parsed.waypoints.hell.is_active(wp), waypoints.hell().is_active(wp));
    }
}

macro_rules! assert_character_common {
    ($parsed:expr, $c:expr) => {{
        let p: &Character = &$parsed.character;
        let c = &$c;
        assert_eq!(p.class, c.class());
        assert_eq!(p.level, c.level());
        assert_eq!(p.flags, c.flags());
        assert_eq!(p.num_stats, c.num_stats());
        assert_eq!(p.num_skills, c.num_skills());
        assert_eq!(p.weapon_switch, c.weapon_switch());
        assert_eq!(p.create_time, c.create_time());
        assert_eq!(p.last_save_time, c.last_save_time());
        assert_eq!(p.play_time, c.play_time());
        assert_eq!(p.map_seed, c.map_seed());
        assert_eq!(p.flags.contains(CharacterFlags::HARDCORE), c.is_hardcore());
        assert_eq!(p.flags.contains(CharacterFlags::DEAD), c.is_dead());
        assert_eq!(p.flags.contains(CharacterFlags::EXPANSION), c.is_expansion());
        assert_eq!(p.flags.contains(CharacterFlags::LADDER), c.is_ladder());
        assert_eq!(&p.appearance_graphics, c.appearance_graphics());
        assert_eq!(&p.appearance_tints, c.appearance_tints());
        assert_eq!(&p.town_difficulty, c.town_difficulty());
        assert_merc(&p.merc, c.merc());
        for i in 0..16 {
            assert_eq!(p.skill_hotkeys[i].skill_id, c.skill_hotkey(i).skill_id());
            assert_eq!(p.skill_hotkeys[i].side, c.skill_hotkey(i).side());
            assert_eq!(p.skill_hotkeys[i].item_index, c.skill_hotkey(i).item_index());
        }
        assert_eq!(p.left_skill.skill_id, c.left_skill().skill_id());
        assert_eq!(p.right_skill.skill_id, c.right_skill().skill_id());
        assert_eq!(p.switch_left_skill.skill_id, c.switch_left_skill().skill_id());
        assert_eq!(p.switch_right_skill.skill_id, c.switch_right_skill().skill_id());
        assert_preview_item(&p.preview.left_hand, c.preview().left_hand());
        assert_preview_item(&p.preview.right_hand, c.preview().right_hand());
        assert_preview_item(&p.preview.torso, c.preview().torso());
        assert_preview_item(&p.preview.head, c.preview().head());
        assert_eq!(p.preview.reserved, c.preview().reserved());
        assert_eq!(p.preview.game_version, c.preview().game_version());
    }};
}

#[test]
fn overlay_matches_parsed_model() {
    for path in V103_SAVES {
        let data = resource(path);
        let parsed = D2Save::from_bytes(&data).unwrap();
        let overlay = D2SaveLayout::new(&data[..]).unwrap();

        assert_eq!(parsed.version, overlay.header().version());
        assert_eq!(parsed.file_size, overlay.header().file_size());
        assert_eq!(parsed.checksum, overlay.header().checksum());

        let expected_name = if parsed.version > 97 { &parsed.character.preview.name } else { &parsed.character.name };
        assert_eq!(expected_name.as_str(), overlay.name());

        let character = overlay.character();
        assert_character_common!(parsed, character);
        let preview = character.preview();
        assert_eq!(parsed.character.preview.expansion_save_time(), preview.expansion_save_time());
        assert_eq!(parsed.character.preview.classic_save_time(), preview.classic_save_time());
        assert_eq!(parsed.character.preview.guild_emblem_color, preview.guild_emblem_color());
        assert_eq!(parsed.character.preview.expansion_experience(), preview.expansion_experience());
        assert_eq!(parsed.character.preview.classic_experience(), preview.classic_experience());

        assert_eq!(parsed.quests.version, overlay.quests().version());
        assert_eq!(parsed.quests.section_size, overlay.quests().section_size());
        assert_waypoints(&parsed, overlay.waypoints());
        assert_eq!(parsed.player_intro.section_size, overlay.player_intro().section_size());

        // Quest flags at a few slots.
        assert_eq!(parsed.quests.normal.act1.den_of_evil, overlay.quests().normal().quest(1, 1));
        assert_eq!(parsed.quests.hell.act5.akara_respec, overlay.quests().hell().quest(5, 9));
    }
}

#[test]
fn overlay_modify_and_verify() {
    for path in ["96/Soska.d2s", "99/Soska.d2s"] {
        let mut data = resource(path);
        let mut overlay = D2SaveLayout::new(&mut data[..]).unwrap();
        let mut character = overlay.character_mut();
        let mut merc = character.merc_mut();
        merc.set_experience(12345678);
        merc.set_hireling_id(999);
        overlay.update_checksum();

        let parsed = D2Save::from_bytes(&data).unwrap();
        assert_eq!(parsed.character.merc.experience, 12345678);
        assert_eq!(parsed.character.merc.hireling_id, 999);
        assert!(D2Save::verify_checksum(&data));
    }
}

#[test]
fn overlay_name_property() {
    for path in ["96/Soska.d2s", "97/Amazon.d2s", "99/Soska.d2s"] {
        let mut data = resource(path);
        let mut overlay = D2SaveLayout::new(&mut data[..]).unwrap();
        assert!(!overlay.name().is_empty());
        overlay.set_name("TestName").unwrap();
        overlay.update_checksum();
        assert_eq!(overlay.name(), "TestName");
        let version = overlay.header().version();

        let parsed = D2Save::from_bytes(&data).unwrap();
        if version > 97 {
            assert_eq!(parsed.character.preview.name, "TestName");
        } else {
            assert_eq!(parsed.character.name, "TestName");
        }
        assert!(D2Save::verify_checksum(&data));
    }
}

#[test]
fn overlay_name_setter_immediate_readback_utf8() {
    let mut data = resource("99/Soska.d2s");
    let name = "Ahą闱";
    let mut layout = D2SaveLayout::new(&mut data[..]).unwrap();
    layout.set_name(name).unwrap();
    assert_eq!(layout.name(), name);
    let save = D2Save::from_bytes(&data).unwrap();
    assert_eq!(save.character.preview.name, name);
}

#[test]
fn overlay_name_setter_immediate_readback_ascii() {
    let mut data = resource("96/Soska.d2s");
    let name = "Ahą闱";
    let mut layout = D2SaveLayout::new(&mut data[..]).unwrap();
    layout.set_name(name).unwrap();
    // Non-ASCII characters cannot be stored in the ASCII name field.
    assert_ne!(layout.name(), name);
    assert_eq!(layout.name(), "Ah??");
    let save = D2Save::from_bytes(&data).unwrap();
    assert_ne!(save.character.preview.name, name);
    assert_ne!(save.character.name, name);
}

#[test]
fn overlay_unlock_all_waypoints() {
    let mut data = resource("97/Amazon.d2s");
    let mut overlay = D2SaveLayout::new(&mut data[..]).unwrap();
    overlay.waypoints_mut().unlock_all();
    overlay.update_checksum();

    let parsed = D2Save::from_bytes(&data).unwrap();
    for wp in 0..39 {
        assert!(parsed.waypoints.normal.is_active(wp), "normal waypoint {wp}");
        assert!(parsed.waypoints.nightmare.is_active(wp), "nightmare waypoint {wp}");
        assert!(parsed.waypoints.hell.is_active(wp), "hell waypoint {wp}");
    }
    assert!(D2Save::verify_checksum(&data));
}

#[test]
fn overlay_v104_matches_parsed_model() {
    for path in V104_SAVES {
        let data = resource(path);
        let parsed = D2Save::from_bytes(&data).unwrap();
        let overlay = D2SaveLayoutV104::new(&data[..]).unwrap();

        assert_eq!(parsed.version, overlay.header().version());
        assert_eq!(parsed.file_size, overlay.header().file_size());
        assert_eq!(parsed.checksum, overlay.header().checksum());
        assert_eq!(parsed.character.preview.name, overlay.name());

        let character = overlay.character();
        assert_character_common!(parsed, character);
        let preview = character.preview();
        assert_eq!(parsed.character.preview.guild_emblem_color, preview.guild_emblem_color());
        assert_eq!(parsed.character.preview.game_version, preview.game_version());
        assert_eq!(parsed.character.preview.preview_padding, preview.preview_padding());
        for i in 0..6 {
            assert_eq!(parsed.character.preview.save_times[i], preview.save_time(i));
            assert_eq!(parsed.character.preview.experiences[i], preview.experience(i));
        }

        assert_eq!(parsed.quests.version, overlay.quests().version());
        assert_eq!(parsed.quests.section_size, overlay.quests().section_size());
        assert_waypoints(&parsed, overlay.waypoints());
        assert_eq!(parsed.player_intro.section_size, overlay.player_intro().section_size());
    }
}

#[test]
fn overlay_v104_modify_and_verify() {
    for path in ["105/Soska.d2s", "105/Roka.d2s"] {
        let mut data = resource(path);
        let mut overlay = D2SaveLayoutV104::new(&mut data[..]).unwrap();
        let mut character = overlay.character_mut();
        let mut merc = character.merc_mut();
        merc.set_experience(12345678);
        merc.set_hireling_id(999);
        overlay.update_checksum();

        let parsed = D2Save::from_bytes(&data).unwrap();
        assert_eq!(parsed.character.merc.experience, 12345678);
        assert_eq!(parsed.character.merc.hireling_id, 999);
        assert!(D2Save::verify_checksum(&data));
    }
}

#[test]
fn overlay_v104_name_property() {
    for path in ["105/Soska.d2s", "105/Roka.d2s"] {
        let mut data = resource(path);
        let mut overlay = D2SaveLayoutV104::new(&mut data[..]).unwrap();
        assert!(!overlay.name().is_empty());
        overlay.set_name("TestName").unwrap();
        overlay.update_checksum();
        assert_eq!(overlay.name(), "TestName");

        let parsed = D2Save::from_bytes(&data).unwrap();
        assert_eq!(parsed.character.preview.name, "TestName");
        assert!(D2Save::verify_checksum(&data));
    }
}

#[test]
fn overlay_v104_new_fields() {
    for path in ["105/Soska.d2s", "105/Roka.d2s"] {
        let data = resource(path);
        let parsed = D2Save::from_bytes(&data).unwrap();
        let overlay = D2SaveLayoutV104::new(&data[..]).unwrap();
        let character = overlay.character();
        let preview = character.preview();
        assert_eq!(parsed.character.preview.game_version, preview.game_version());
        assert_eq!(parsed.character.preview.expansion_save_time(), preview.save_time(0));
        assert_eq!(parsed.character.preview.classic_save_time(), preview.save_time(1));
        assert_eq!(parsed.character.preview.expansion_experience(), preview.experience(0));
        assert_eq!(parsed.character.preview.classic_experience(), preview.experience(1));
    }
}

#[test]
fn overlay_rejects_wrong_layout() {
    for path in V104_SAVES {
        assert!(D2SaveLayout::new(&resource(path)[..]).is_err(), "{path}");
    }
    for path in V103_SAVES {
        assert!(D2SaveLayoutV104::new(&resource(path)[..]).is_err(), "{path}");
    }
}

#[test]
fn overlay_v104_unlock_all_waypoints() {
    for path in ["105/Soska.d2s", "105/Roka.d2s"] {
        let mut data = resource(path);
        let mut overlay = D2SaveLayoutV104::new(&mut data[..]).unwrap();
        overlay.waypoints_mut().unlock_all();
        overlay.update_checksum();

        let parsed = D2Save::from_bytes(&data).unwrap();
        for wp in 0..39 {
            assert!(parsed.waypoints.normal.is_active(wp));
            assert!(parsed.waypoints.nightmare.is_active(wp));
            assert!(parsed.waypoints.hell.is_active(wp));
        }
        assert!(D2Save::verify_checksum(&data));
    }
}

#[test]
fn save_layout_picks_version() {
    for path in V103_SAVES.iter().chain(&V104_SAVES) {
        let mut data = resource(path);
        let parsed = D2Save::from_bytes(&data).unwrap();
        assert_eq!(save_version(&data).unwrap(), parsed.version);

        let mut layout = SaveLayout::new(&mut data[..]).unwrap();
        assert_eq!(layout.version(), parsed.version);
        assert_eq!(layout.level(), parsed.character.level);
        assert_eq!(layout.class(), parsed.character.class);
        layout.set_level(42);
        layout.merc_mut().set_hireling_id(777);
        layout.update_checksum();

        let parsed = D2Save::from_bytes(&data).unwrap();
        assert_eq!(parsed.character.level, 42);
        assert_eq!(parsed.character.merc.hireling_id, 777);
        assert!(D2Save::verify_checksum(&data));
    }
}

#[test]
fn stash_tab_layout() {
    let data = resource("105/ModernSharedStashSoftCoreV2.d2i");
    let stash = D2StashSave::from_bytes(&data).unwrap();
    let mut offset = 0;
    for tab in &stash {
        let layout = D2StashTabLayout::new(&data[offset..]).unwrap();
        assert_eq!(layout.stash_format(), tab.stash_format);
        assert_eq!(layout.save_version(), tab.save_version);
        assert_eq!(layout.gold(), tab.gold);
        assert_eq!(layout.season(), tab.season);
        assert_eq!(layout.tab_type(), tab.tab_type);
        assert_eq!(layout.padding_21(), &tab.padding_21);
        assert_eq!(layout.withdraw_only_data(), &tab.withdraw_only_data);
        assert_eq!(layout.padding_44(), &tab.padding_44);
        assert_eq!(layout.is_withdraw_only(), tab.is_withdraw_only());
        offset += layout.tab_size() as usize;
    }
    assert_eq!(offset, data.len());
    assert!(D2StashTabLayout::new(&data[..10]).is_err());
}
