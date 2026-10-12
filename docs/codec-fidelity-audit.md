# Upstream scope after codec review

Compared D2SSharp `baf955a` with original Rust `1d6c2e7`. All 18 supplied
character/stash fixtures round-trip byte-for-byte through current D2SSharp,
including the modded fixture with its matching tables. No C# source was changed.

Constructed invalid items demonstrate differences in zero-valued stats, repeated
paired groups, optional-value presence, flags, padding, and other raw encodings.
These are not required behavior for standard D2R saves. The user confirmed that
invalid-item scenarios should be excluded from this upstream change.

## Retained changes

- Expose the five existing versioned TSV tables through `GameTable` and
  `TxtData::table`, retaining columns needed by the application's Rust item
  synthesis. No new game tables or synthesis rules are added to this codec.
- Expose optional inventory width and height on `ItemTypeInfo`, from the matching
  version's item row, for item placement constraints.
- Cover custom source text, exact version selection, embedded borrowing and
  lazy parsing, both filesystem loaders, and dimensions across item tables.

## Removed changes

All binary reader/writer changes were removed: serialization metadata, stat
encoding roles, opaque tails/padding, raw presence flags, gold/level encoding,
stash normalization changes, and recursive item-record depth tracking. The
constructed invalid-item regression was also removed. Save models and serialized
JSON retain their original shapes. Socket fillers do not gain nested sockets.

These retained changes are data access capabilities, not codec correctness fixes.
`ItemTypeInfo` gains two public fields; downstream exhaustive struct literals
must add them or use `..Default::default()`. Existing codec tests continue to
cover ordinary save reconstruction. Package version remains unchanged for the
user's release. The save-store application's vendored dependency is unchanged;
removing its now-unnecessary fidelity metadata is a separate consuming-code task.

## Validation

Passed the all-features suite (74 tests including doctests), six library tests
without default features, formatting, Clippy with warnings denied for all targets
and without default features, documentation with warnings denied, and the
Rust 1.80 all-features library build.
