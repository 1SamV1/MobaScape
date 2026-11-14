# MobaScape

A 5v5 MOBA built on a custom Rust engine, inspired by modern Old School RuneScape
for its visual identity. Server-authoritative, data-driven, and
designed for fast-paced low-team-count matches.

> Status: **M5.6 Biome map + Dota style presentation**. The cache reader,
> terrain/location/model/sequence decoders, animated NPCs, a 5x5 Karamja block,
> A* pathfinding with collision, hold-to-steer, a bitmap-font HUD, auto-attacks,
> a Q/W/E/R kit with mana/cooldowns/XP/gold, three data-driven champions, a
> six-item shop with tooltips and six inventory slots, passive gold and
> fountain healing, three lanes with towers/inhibitors/shielded nexus,
> inhibitor respawn and super minions, minion waves, jungle camps with team
> buffs, brush/fog of war, a fogged minimap with click-to-pan, and a win
> condition are working. **117HD Core** adds sRGB-correct sun/ambient
> lighting, per-face model normals, smooth terrain lighting, a sky with
> clouds, height fog, static and unit blob shadows, and dynamic
> ability/fountain point lights. **M5.1** adds a 1024px sun shadow map with
> PCF, dynamic player/structure/camp casters, alternating shadow updates, and
> baked contact AO. The **UI pass** adds an OSRS-style Fountain Shop window
> with categories, item cards, detail/buy/sell panes, real item icons and tab
> sprites, plus HUD objectives, team, resource, event-log, inventory, and
> shop-button panels. **M5.3/M5.4** adds seamless terrain color smoothing,
> procedural ground detail, and an offscreen post pass with FXAA, color
> grading, and dithering. **M5.5** replaces the cheap glow with a full HDR
> bloom chain (Rgba16Float scene target, bright pass, separable Gaussian,
> ACES tonemap) and samples real OSRS ground sprites from a mipmapped texture
> array. **M5.6** rebuilds the map as a two-sided rift: eight regional biome
> pockets inspired from OSRS regions (dark magic, vampires, ogres, goblins on
> the dire side; human cities, elven lands, gnome stronghold on the radiant
> side), a diagonal river with a centre bridge, tree lines down every lane, a
> shadow (not tinted) fog of war, and a Dota style scoreboard, unit tooltip,
> minimap viewport and biome label. See [docs/STATUS.md](docs/STATUS.md) and
> [docs/UI_ASSETS.md](docs/UI_ASSETS.md).

## What exists

- `crates/cache` — read-only cache reader:
  - 520-byte sector store, 6-byte index entries, XTEA, gzip/bzip2/LZMA
  - index reference tables, archive splitting with real file IDs
  - terrain (64×64×4), locations, object/NPC/underlay/overlay configs
  - model formats 2 and 3 with flat-shading normals and vertex groups
  - sequences, frames, framemaps, and interpolated vertex animation
- `crates/sim` — deterministic tile grid, collision, and A* pathfinding
- `crates/tools` — `rift-tools` CLI for cache inspection and OBJ export
- `crates/client` — native wgpu client: 5x5 Karamja with regional biome
  pockets and a river, animated NPCs, three lanes, towers/inhibitors/nexus,
  minion waves, jungle camps/buffs with aggro and leash, brush/fog of war,
  item shop/inventory, bitmap-font HUD, 117HD Core lighting/atmosphere, HDR
  bloom, cache-native ground textures, fogged minimap, click-to-move +
  hold-to-steer, auto-attacks, Q
- `data/maps/karamja_rift/map.json` — lanes, structures, camps, biome pockets,
  river and wave timing
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
cargo run -p rift-tools -- sprite 447 /tmp/wood.png
cargo run -p rift-tools -- texture 60 /tmp/leaves.png

# Render the 5x5 block and open the hero-select screen
cargo run -p rift-client

# Skip hero select with an explicit champion
cargo run -p rift-client -- --champion monkey

# Deterministic combat check: target a dummy and fire Q
cargo run -p rift-client -- --demo-combat

# Fast-forward the lane for tower/minion/nexus verification
cargo run -p rift-client -- --timescale 20

# Teleport to a jungle camp, kill it, and verify the team buff
cargo run -p rift-client -- --demo-camp --timescale 4

# Cast W/E/R and print mana/cooldown/XP state
cargo run -p rift-client -- --demo-kit --timescale 4

# Play a different champion
cargo run -p rift-client -- --champion jad
cargo run -p rift-client -- --champion kree

# Zoom the camera in on the hero (the default distance is 2750 world units)
cargo run -p rift-client -- --distance 1800

# Spawn at a specific map tile (handy for inspecting a watchtower)
cargo run -p rift-client -- --spawn 2911 3032

# Verify watchtower aggro, lock range and target reset
cargo run -p rift-client -- --demo-watchtower

# 117HD Core lighting (default) or the flat classic baseline
cargo run -p rift-client -- --graphics 117hd
cargo run -p rift-client -- --graphics classic

# Benchmark/quality overrides
cargo run --release -p rift-client -- --present-mode immediate
cargo run --release -p rift-client -- --msaa 1

# Inspect lighting, normals, albedo, fog, shadows, or AO
cargo run -p rift-client -- --graphics 117hd --debug-view lighting
cargo run -p rift-client -- --graphics 117hd --debug-view normal
cargo run -p rift-client -- --graphics 117hd --debug-view shadow
cargo run -p rift-client -- --graphics 117hd --debug-view ao
cargo run -p rift-client -- --graphics 117hd --shadows off|blob|map
cargo run -p rift-client -- --graphics 117hd --pitch 0.22

# Optional: populate the gitignored UI asset tree (see docs/UI_ASSETS.md)
# assets/osrs-ui/items/*.png + assets/osrs-ui/ui/tab.png

# Grant gold, open the shop, and buy all six items
cargo run -p rift-client -- --demo-shop --timescale 4

# Verify fountain healing and passive gold
cargo run -p rift-client -- --demo-base --timescale 4

# Destroy an inhibitor, verify super minions and respawn
cargo run -p rift-client -- --demo-inhibitor --timescale 20
```

The cache download is not automated yet; see
[docs/ASSET_PIPELINE.md](docs/ASSET_PIPELINE.md) for the exact URLs and layout.
Put `keys.json` next to `main_file_cache.dat2`, or pass `--keys`.

## Controls (viewer)

- **Hero select**: click a card or press **1-3**; **Enter**/click **Play** to lock in
- **LMB ground tap**: pathfind to the point
- **LMB hold**: steer directly toward the cursor with collision sliding
- **LMB on an enemy**: auto-attack it
- **Q**: fire blast projectile (5 s)
- **W**: blink/dash toward the cursor (8 s)
- **E**: heal + shield (12 s)
- **R**: meteor AoE at the cursor (45 s)
- **B**: toggle the Fountain Shop
- **Click a category**: filter shop items
- **Click an item card**: select it; use **Buy** or **Sell** in the detail pane
- **1–6**: quick-buy the matching shop item while the shop is open
- **Click the minimap**: pan the camera to that point (disables follow)
- **Arrow keys**: pan the camera (disables follow)
- **Z / X**: rotate camera
- **Space**: recenter/follow
- `Ctrl` + wheel: zoom (plain wheel is ignored to avoid accidental zoom)
- `F`: toggle camera follow
- `Esc`: close the shop, or quit

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
- [docs/ENGINE_ROADMAP.md](docs/ENGINE_ROADMAP.md)
- [docs/MAP_FORMAT.md](docs/MAP_FORMAT.md)
- [docs/NETCODE.md](docs/NETCODE.md)
- [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md)
- [docs/UI_ASSETS.md](docs/UI_ASSETS.md)
- [docs/DECISIONS.md](docs/DECISIONS.md)

## License

MIT. OSRS cache data and Jagex assets are not part of this repository and are
not covered by the MIT license. Do not redistribute extracted game assets.
