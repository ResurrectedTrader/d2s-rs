//! JSON round-trip of a parsed save (requires the `serde` feature).

mod common;

use common::resource;
use d2s::{D2Save, D2StashSave};

#[test]
fn round_trip_json_serialization() {
    let bytes = resource("99/Roka.d2s");
    let save = D2Save::from_bytes(&bytes).unwrap();
    let json = serde_json::to_string_pretty(&save).unwrap();
    let restored: D2Save = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, save);
    assert_eq!(restored.to_bytes().unwrap(), bytes, "JSON round-trip produced different bytes");
}

#[test]
fn round_trip_json_all_saves() {
    for path in ["96/Soska.d2s", "97/Druid.d2s", "105/ChaosSC.d2s", "105/Roka.d2s"] {
        let bytes = resource(path);
        let save = D2Save::from_bytes(&bytes).unwrap();
        let restored: D2Save = serde_json::from_str(&serde_json::to_string(&save).unwrap()).unwrap();
        assert_eq!(restored.to_bytes().unwrap(), bytes, "{path}");
    }
    let bytes = resource("105/ModernSharedStashSoftCoreV2.d2i");
    let stash = D2StashSave::from_bytes(&bytes).unwrap();
    let restored: D2StashSave = serde_json::from_str(&serde_json::to_string(&stash).unwrap()).unwrap();
    assert_eq!(restored.to_bytes().unwrap(), bytes);
}
