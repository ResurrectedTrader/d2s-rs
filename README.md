# d2s

[![CI](https://github.com/ResurrectedTrader/d2s-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/ResurrectedTrader/d2s-rs/actions/workflows/ci.yml)
[![Publish](https://github.com/ResurrectedTrader/d2s-rs/actions/workflows/publish.yml/badge.svg?branch=main)](https://github.com/ResurrectedTrader/d2s-rs/actions/workflows/publish.yml)
[![crates.io](https://img.shields.io/crates/v/d2s.svg)](https://crates.io/crates/d2s)
[![docs.rs](https://docs.rs/d2s/badge.svg)](https://docs.rs/d2s)

A Rust library for reading and writing Diablo 2 save files: `.d2s` character saves and
`.d2i` shared stash files. Supports the original Lord of Destruction format (1.10+, save
version 96) and Diablo 2 Resurrected (save versions 97-105).

> This crate is a Rust port of the C# library
> **[D2SSharp](https://github.com/ResurrectedTrader/D2SSharp)**
> ([NuGet](https://www.nuget.org/packages/D2SSharp)). The two produce byte-identical output
> for the same input, and fixes are kept in sync between them.

## Features

- Full read/write support for character saves (.d2s) and shared stash files (.d2i)
- D2 LOD (version 96) and D2R (versions 97-105) formats
- Version conversion across all format boundaries (v96 ↔ v97+, v<=103 ↔ v104+, v<=104 ↔ v105+)
- Complete item parsing: stats, sockets, runewords, set bonuses, ears, chronicle data
- Shared stash tab types: Normal, AdvancedStash and Chronicle
- Demon section (v103+, Reign of the Warlock)
- Exact round-tripping: reading and writing an unmodified file produces identical bytes,
  including unknown enum values, padding and trailing mod data
- Zero-copy [overlay API](#overlay-api-zero-copy-access) for editing header fields without parsing the whole save
- Loading modded game data from .txt files
- No `unsafe` code; the only dependency is `bitflags` (plus optional `serde`)

## Usage

```toml
[dependencies]
d2s = "0.1"
```

### Reading a character save

```rust
use d2s::{D2Save, StatId};

let bytes = std::fs::read("MyCharacter.d2s")?;
let save = D2Save::from_bytes(&bytes)?;

println!("Character: {}", save.name());
println!("Level: {}", save.character.level);
println!("Class: {:?}", save.character.class);
println!("Strength: {}", save.stats.get(StatId::STRENGTH));

for item in &save.items {
    println!("{} ({:?}, level {})", item.code, item.quality, item.level);
    for stat in &item.stats {
        println!("  {} = {}", stat.id, stat.value);
    }
    for socketed in item.socketed_items() {
        println!("  socketed: {}", socketed.code);
    }
}
```

### Modifying and saving

```rust
use d2s::{D2Save, Stat, StatId};

let mut save = D2Save::from_bytes(&std::fs::read("MyCharacter.d2s")?)?;

save.stats.set(StatId::STRENGTH, 200);
save.stats.set(StatId::STASH_GOLD, 2_500_000);
save.character.level = 99;
save.skills[0] = 20;
save.waypoints.unlock_all();

if let Some(item) = save.items.first_mut() {
    item.stats.push(Stat::new(StatId::MAGIC_FIND, 25));
}

// File size and checksum are computed on write.
std::fs::write("MyCharacter.d2s", save.to_bytes()?)?;

// Or reuse a buffer between writes.
let mut buf = Vec::new();
save.write_to(&mut buf, d2s::TxtData::embedded())?;
```

### Reading a shared stash

```rust
use d2s::{D2StashSave, StashTabType};

let stash = D2StashSave::from_bytes(&std::fs::read("SharedStashSoftCoreV2.d2i")?)?;
for tab in &stash {
    match (&tab.tab_type, &tab.chronicle) {
        (StashTabType::Chronicle, Some(chronicle)) => {
            println!("chronicle: {} set entries", chronicle.set_entries.len())
        }
        _ => println!("{:?} tab: {} items, {} gold", tab.tab_type, tab.items.len(), tab.gold),
    }
}
```

## Overlay API (zero-copy access)

For simple edits of the fixed-size header sections, the overlay API reads and writes fields
directly in the file buffer without parsing it. Layouts are generic over their storage: use
`&[u8]` for read-only access and `&mut [u8]` (or `Vec<u8>`) for modification.

```rust
use d2s::overlay::SaveLayout;

let mut data = std::fs::read("MyCharacter.d2s")?;
let mut layout = SaveLayout::new(&mut data[..])?; // picks the v<=103 or v104+ layout

println!("{} level {}", layout.name(), layout.level());
layout.set_name("NewName")?;
layout.set_level(99);
layout.waypoints_mut().unlock_all();
layout.update_checksum();

std::fs::write("MyCharacter.d2s", &data)?;
```

The version-specific layouts give access to every header field:

```rust
use d2s::overlay::{save_version, D2SaveLayout, D2SaveLayoutV104};

let mut data = std::fs::read("MyCharacter.d2s")?;
if save_version(&data)? >= 104 {
    let mut layout = D2SaveLayoutV104::new(&mut data[..])?;
    let character = layout.character();
    let preview = character.preview();
    println!("{:?} save time {}", preview.game_version(), preview.save_time(0));
    layout.character_mut().merc_mut().set_experience(1_000_000);
    layout.update_checksum();
} else {
    let mut layout = D2SaveLayout::new(&mut data[..])?;
    layout.quests_mut().hell_mut().set_quest(5, 9, d2s::QuestFlags::REWARD_PENDING);
    layout.update_checksum();
}
```

The overlay covers the header, character, preview, mercenary, quest, waypoint and NPC
intro sections (765 bytes for v<=103, 833 bytes for v>=104). Stats, skills and items require
full parsing. [`D2StashTabLayout`](src/overlay.rs) gives the same access to stash tab headers.

### Performance

Measured with `cargo bench` on a level 99 character with a full inventory (`tests/resources/99/Roka.d2s`):

| Operation                   | Time    |
|-----------------------------|---------|
| Full deserialize            | 11.5 µs |
| Full serialize              | 11.6 µs |
| Full round-trip             | 23.3 µs |
| Overlay: read name          | 8 ns    |
| Overlay: modify + checksum  | 0.94 µs |

## Save format versions

| Version | Game         | Notes                                                              |
|---------|--------------|--------------------------------------------------------------------|
| 96      | D2 LOD 1.10+ | 32-bit item codes, 7-bit strings                                   |
| 97      | D2R 2.0      | Huffman-encoded item codes, 7-bit strings                          |
| 98-99   | D2R 2.8      | Huffman-encoded item codes, 8-bit strings                          |
| 100-102 | D2R 3.0      | Advanced stash tab types, chronicle data, item find tracking       |
| 103     | D2R 3.0      | Demon section for summoned creature persistence                    |
| 104     | D2R 3.0      | New header layout (833 bytes), name moved to preview, game version |
| 105     | D2R 3.0      | Item quantity uses a 1-bit presence flag for all items             |

## Version conversion

`convert_to` changes a save to another format in place; the next write uses the new format.

```rust
use d2s::D2Save;

let mut save = D2Save::from_bytes(&std::fs::read("old_character.d2s")?)?; // version 96
save.convert_to(105);
std::fs::write("new_character.d2s", save.to_bytes()?)?;
```

Conversions handle:

- **1.14 → D2R**: the name moves to the UTF-8 preview name, preview items are built from
  equipped items (with transforms from the appearance tints), and body locations of
  non-equipped items are cleared (D2R rejects them).
- **D2R → 1.14**: the preview name moves back to the character name and preview items are cleared.
- **v<=103 ↔ v104+**: the name field moves into the preview data, the preview grows to six
  save time / experience slots, and the expansion flag becomes the game version.
- **v<=104 ↔ v105+**: item quantities switch to the presence-flag format.

Stash files convert the same way with `D2StashSave::convert_to`, which also adjusts the stash
format (tab types require stash format 2, v100+).

## External data

Parsing items requires game data: stat bit widths from ItemStatCost.txt and item
classifications from the item tables. Data for versions 96, 97, 99 and 105 is embedded
(feature `embedded-data`, on by default) and parsed lazily per version. For modded games,
load your own .txt files:

```rust
use d2s::{D2Save, TxtData};

// A directory with version subdirectories: MyTxtFiles/99/{armor,itemstatcost,itemtypes,misc,weapons}.txt
let data = TxtData::from_dir("MyTxtFiles")?;
// Or a single directory for one version.
let data = TxtData::from_version_dir("MyTxtFiles/99", 99)?;

let save = D2Save::from_bytes_with(&std::fs::read("modded.d2s")?, &data)?;
let bytes = save.to_bytes_with(&data)?;
```

Data is selected by exact save version; reading a version without data fails with
`Error::UnsupportedVersion`. You can also implement the `ExternalData` trait yourself.
Disable default features to drop the embedded tables (~1.2 MB) if you always supply your own.

## Cargo features

| Feature         | Default | Description                                                      |
|-----------------|---------|------------------------------------------------------------------|
| `embedded-data` | yes     | Embedded game tables and the `from_bytes`/`to_bytes` conveniences |
| `serde`         | no      | `Serialize`/`Deserialize` for all model types                     |

## Mod compatibility

- **Custom item data**: load mod .txt files as shown above.
- **Trailing data**: bytes after the last known section are kept in `D2Save::trailing_data`
  and written back unchanged.
- Saves must contain all standard sections; modified section layouts are not supported.

## Building

```bash
cargo build
cargo test --all-features
cargo bench
```

## Related projects

- [D2SSharp](https://github.com/ResurrectedTrader/D2SSharp): the C# library this crate is ported from.

## Acknowledgments

- [D2MOO](https://github.com/ThePhrozenKeep/D2MOO)
- dschu012 for [d2s](https://github.com/dschu012/d2s)
- Killshot
- BRE

## License

MIT
