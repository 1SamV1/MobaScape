# Asset pipeline

This document is the operational guide for getting modern OSRS assets into the
engine. It records what we verified against the pinned cache, what the decoders
currently support, and which external tools are useful when our decoder is not
enough.

## 1. Pinned cache

Development uses **OpenRS2 cache 2392, build 235 (2025-12-17)**:

- `https://archive.openrs2.org/caches/runescape/2392/disk.zip`
- `https://archive.openrs2.org/caches/runescape/2392/keys.json`
- 167 MB compressed cache, 2,668 valid XTEA keys.

Why this snapshot: builds 241+ on OpenRS2 currently report **0 archived XTEA
keys**, which blocks map-location extraction. Build 235 is recent enough to
include modern content and has a complete key set. Do not chase "latest" until
the key situation is understood.

Layout used by the tools:

```
.cache/osrs-235/cache/
  main_file_cache.dat2
  main_file_cache.idx0 ... idx24, idx255
  keys.json
```

The cache is gitignored. The repo ships the reader and tools, not the data.

### Cache 2757 / build 242 note

Cache 2757 / build 242 is a useful **character/model** source (per the
community quickstart), but it is not a permanent "latest" target. If we need
newer character content than 235, treat it as a second, explicitly versioned
source and keep model/sequence IDs and hashes with the asset. Do not mix IDs
from different builds silently.

## 2. Verified cache format

Our Rust reader (`crates/cache`) implements the following. These details were
verified against cache 2392 with Python and RuneLite's cache library.

### Store

- `main_file_cache.dat2`: 520-byte sectors; 512 data bytes + 8-byte header.
- Index file entry: **6 bytes** — 3-byte group length, 3-byte first sector.
- Sector header: `group id` u16, `chunk index` u16, `next sector` u24,
  `next chunk` u8.
- Group header: compression u8, compressed length u32.
- The decompressed length follows, then the compressed payload.
- For encrypted map groups, XTEA starts at **byte 5** (after compression and
  compressed length) and includes the 4-byte decompressed length.
- Compression:
  - `0` none
  - `1` bzip2 — the `BZh<level>` stream header is stripped; prepend `BZh9`
  - `2` gzip — the 8-byte gzip trailer is damaged/foreign; decompress the raw
    deflate stream and ignore the trailer
  - `3` LZMA — implemented with `lzma-rs`, not exercised by cache 2392

### Index map (expected, verified in M0 for the used indices)

| Index | Contents |
|---|---|
| 0 | animation frames |
| 1 | frame sets |
| 2 | configs (underlay 1, identkit 3, overlay 4, object 6, npc 9, item 10, sequence 12, spotanim 13, …) |
| 3 | interfaces |
| 4 | sounds |
| 5 | maps (terrain + locations) |
| 6 | music |
| 7 | models |
| 8 | sprites |
| 9 | textures |
| 255 | reference tables |

Map groups are named `m{x}_{y}` (terrain) and `l{x}_{y}` (locations) and are
stored in pairs; the locations group is `terrain + 1`. `keys.json` gives the
locations group name and XTEA key.

### Reference tables

Index 255 group `i` decompresses to `IndexData` for index `i`:

- protocol 5–7, revision, flags (named, sized)
- archive IDs (delta-encoded), name hashes, CRCs, sizes, revisions
- per-archive file counts and file IDs
- per-file name hashes when named

`Cache::archive_files` splits multi-file archives using the chunk table at the
end of the decompressed group: last byte = chunk count, then `chunks × files × 4`
bytes of cumulative size deltas, then interleaved file data.

## 3. What our decoders support

| Data | Status | Notes |
|---|---|---|
| Terrain | ✅ | 4 planes × 64×64, attribute opcodes, height/overlay/underlay/settings |
| Locations | ✅ | object ID deltas, packed position, shape, rotation, plane |
| Object configs | ✅ | full opcode parser including params and conditional ops |
| Underlay/overlay | ✅ | RGB + texture ID, hide-underlay flag |
| Models type 2 | ✅ | positions, faces, colors, render types, textures, groups |
| Models type 3 | ✅ | positions, faces, colors, textures, groups, v12 normals |
| Models type 1 / old | ❌ | return a clear error; add if a needed model uses them |
| Sequences/frames | ❌ | next milestone; frames/frame sets in indices 0/1, sequence configs in 2/12 |
| Sprites/icons | ❌ | index 8; next milestone |
| Textures | ❌ | index 9; deferred until flat-shaded content is complete |
| Sounds/music | ❌ | indices 4/6; v1 uses placeholder SFX |

Model type detection: last two bytes are `ff fd` (type 3), `ff fe` (type 2),
`ff ff` (type 1), otherwise old format. Type 2 and 3 headers are read from the
tail and blocks are addressed by explicit offsets, not sequential parsing.

## 4. Working commands

```sh
cargo run -p rift-tools -- cache-info
cargo run -p rift-tools -- terrain 44 47
cargo run -p rift-tools -- locations 44 47
cargo run -p rift-tools -- underlay 50
cargo run -p rift-tools -- overlay 16
cargo run -p rift-tools -- object 1282
cargo run -p rift-tools -- find-object tree --limit 10
cargo run -p rift-tools -- model 1715
cargo run -p rift-tools -- export-obj 1715 /tmp/tree.obj
cargo run -p rift-tools -- terrain-obj 44 47 /tmp/terrain.obj
```

Use Blender to inspect exported OBJs quickly:

```sh
blender /tmp/tree.obj
```

## 5. External fallback tools

Use these to unblock or cross-check our decoder. They are not runtime
dependencies.

- **osrscachereader** (JS, BSD-2-Clause): cache reading, model/GLTF export,
  animation loading, XTEA. Good for a second opinion on model/sequence layout.
- **RuneLite cache library** (Java, BSD-2-Clause): the reference for model,
  terrain, location, config, sequence, and sprite decoders.
- **OpenRS2**: cache snapshots, keys, reference tables, archive metadata.
- **OSRS Environment Exporter**: terrain + placed objects to glTF. The tested
  older combo is exporter 2.4.2 + cache 1703/build 219; newer caches can produce
  malformed geometry with older decoders. Verify one region visually.
- **rs-map-viewer**: scene and collision decoding reference.
- **RuneLite Model Exporter**: meshes and sampled OBJ animation frames to
  `.runelite/models/`; disable animation smoothing when collecting source
  frames.
- **RuneMonk**: inspection; animation lists can lag the cache.
- **skelspanels.com/sprites**: PNG sprites if cache sprite decoding is not ready.
  Record the pack date/snapshot so IDs stay traceable.

Known caveats from the community quickstart:

- Modern content may use **Animaya** animations; our type 3 decoder reads the
  flag but does not yet apply Animaya data.
- The inspected glTF exporter bakes animation into morph targets without an
  editable skeleton. Our engine uses vertex animation frames, so this is fine for
  playback but not for retargeting.
- A successful file export does not prove correctness. Verify colours, textures,
  and animation visually against the game.

## 6. Visual verification checklist

For every new asset class:

1. Export one asset (e.g. a tree, an NPC, one terrain region).
2. Compare silhouette, scale, colours, and facing against RuneLite or an OSRS
   screenshot.
3. If animated, play the full sequence and check looping and root motion.
4. Record cache ID, model ID, sequence ID, and source build in the PR/test note.
5. Check the asset in engine space: 1 tile = 128 units, height ×8, Y flipped.

## 7. Legal and distribution

Jagex owns the cache and assets. This repository does **not** include them.
Users must obtain their own cache and run the extractor locally. Do not
redistribute extracted assets, cache files, or screenshots of game assets in the
repo. Keep the repository private until the distribution policy is decided.
