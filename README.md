# MobaScape

A 5v5 MOBA built on a custom Rust engine, using modern Old School RuneScape
cache assets for its visual identity. Server-authoritative, data-driven, and
designed for fast-paced low-team-count matches.

> Status: **M1 vertical slice**. The cache reader, terrain/location/model/sequence
> decoders, animated NPCs, a 3x3 Karamja block, click-to-move, and a wgpu viewer
> are working. There is no HUD, combat, pathfinding, or networking yet. See
> [docs/STATUS.md](docs/STATUS.md).

## What exists

- `crates/cache` — read-only OSRS cache reader:
  - 520-byte sector store, 6-byte index entries, XTEA, gzip/bzip2/LZMA
  - index reference tables, archive splitting with real file IDs
  - terrain (64×64×4), locations, object/NPC/underlay/overlay configs
  - model formats 2 and 3 with flat-shading normals and vertex groups
  - sequences, frames, framemaps, and interpolated vertex animation
- `crates/tools` — `rift-tools` CLI for cache inspection and OBJ export
- `crates/client` — native wgpu viewer: 3x3 Karamja, scenery, animated NPCs,
  click-to-move player, camera follow
- `docs/reference` — archived Java-era experiments (not built)

## Requirements

- Rust 1.85+ (developed on 1.97)
- Vulkan / Metal / DX12 capable GPU
- An OSRS cache. Development pins OpenRS2 cache **2392 / build 235** because it
  has a complete XTEA key set. The cache is **not** committed.

## Quick start

```sh
# Download and unpack the pinned cache into .cache/osrs-235/cache
cargo run -p rift-tools -- cache-info

# Inspect Karamja (region 44,47)
cargo run -p rift-tools -- terrain 44 47
cargo run -p rift-tools -- locations 44 47
cargo run -p rift-tools -- find-object tree --limit 5
cargo run -p rift-tools -- find-npc monkey --limit 5
cargo run -p rift-tools -- sequence 222
cargo run -p rift-tools -- animate 7744 222
cargo run -p rift-tools -- model 1715

# Render the 3x3 block and play
cargo run -p rift-client
```

The cache download is not automated yet; see
[docs/ASSET_PIPELINE.md](docs/ASSET_PIPELINE.md) for the exact URLs and layout.
Put `keys.json` next to `main_file_cache.dat2`, or pass `--keys`.

## Controls (viewer)

- **LMB ground**: click to move the champion (tap-to-move)
- `WASD` / arrows: pan the camera (disables follow)
- `Q` / `E`: rotate
- `Ctrl` + wheel: zoom (plain wheel is ignored to avoid accidental zoom)
- `F`: toggle camera follow
- `R`: reset camera to the champion
- `Esc`: quit

## Design north star

- **OSRS identity, MOBA pacing.** 128-unit tiles, flat per-face shading,
  cache-native models, but 30 Hz authoritative combat and 5v5 match flow.
- **Total conversion.** The client boots into the MOBA; no normal OSRS play.
- **Humans only in production.** Bots fill dev slots and run headless soak tests.
- **Data-driven content.** Champions, abilities, items, maps, waves, and UI live
  in `data/` and hot-reload.
- **Server authority.** The client predicts movement and plays cosmetics; the
  server owns damage, vision, cooldowns, and match state.

## Documentation for agents

Start with [AGENTS.md](AGENTS.md). It defines the rules, repo map, build/test
commands, and how to contribute without breaking determinism or leaking assets.

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- [docs/ASSET_PIPELINE.md](docs/ASSET_PIPELINE.md)
- [docs/ROADMAP.md](docs/ROADMAP.md)
- [docs/STATUS.md](docs/STATUS.md)
- [docs/MAP_FORMAT.md](docs/MAP_FORMAT.md)
- [docs/NETCODE.md](docs/NETCODE.md)
- [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md)
- [docs/DECISIONS.md](docs/DECISIONS.md)

## License

MIT. OSRS cache data and Jagex assets are not part of this repository and are
not covered by the MIT license. Do not redistribute extracted game assets.
