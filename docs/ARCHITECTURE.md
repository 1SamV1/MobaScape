# Architecture

## Shape of the system

```
OSRS cache (pinned snapshot)
        │
        ▼
crates/cache ──► decoded data structs ──► crates/render / scene builder
        │                                      │
        └──► crates/tools (inspect/export)      ▼
                                          wgpu client
                                                ▲
crates/data (JSON, hot reload) ──► crates/sim ──┘
                                      │
                                      ▼
                               crates/server ──► crates/proto ──► clients
```

The cache reader is a pure, read-only library. It has no wgpu, winit, or game
logic. The simulation is IO-free and deterministic. The server owns a `sim`;
the client owns a predicted local `sim` and a renderer.

## Crates

| Crate | Responsibility |
|---|---|
| `crates/cache` | Cache store, XTEA, compression, reference tables, terrain, locations, configs, models |
| `crates/tools` | `rift-tools` CLI: inspect, validate, dump OBJ, later bake asset packs |
| `crates/client` | winit window, wgpu surface/device, scene builder, camera, input, HUD, combat sandbox |
| `crates/render` (planned) | Shared OSRS-style renderer: chunks, models, animation, fog, water |
| `crates/sim` | Deterministic tile grid, collision, A* pathfinding; grows into the 30 Hz simulation |
| `crates/data` (planned) | Serde content types + file watcher + hot reload |
| `crates/proto` (planned) | Messages, input framing, snapshot delta encoding |
| `crates/server` (planned) | Match instances, lobby, room codes, persistence |
| `crates/ui` (planned) | egui HUD, shop, scoreboard, minimap, lobby |
| `crates/editor` (planned) | Map editor, procedural generation, asset library |
| `crates/bots` (planned) | Dev-only slot fillers and headless soak tests |
| `crates/mcp` (planned) | MCP dev server: screenshot, commands, match state, reload |

## Coordinate system and units

- **1 tile = 128 world units.** This is the OSRS convention and is non-negotiable
  for model and map fidelity.
- **Cache terrain height = ×8 world units.** RuneLite's `Region` uses
  `height * 8`; a cache height of 50 is 400 world units.
- **Engine space is Y-up.** Cache model Y is flipped (`world_y = base_y - model_y`)
  because RS models use negative Y for "up".
- **Region origin:** region `(rx, ry)` covers tiles `rx*64 .. rx*64+63`; world
  `x = tile_x * 128`, `z = tile_y * 128`.
- **Camera:** orbit target, yaw, pitch, distance. Default yaw 0.72 rad, pitch
  0.86 rad, distance 9000 units for the M0 viewer. The MOBA camera will use a
  fixed isometric-ish yaw/pitch with zoom and edge pan.
- **Model scale:** 1 cache unit = 1 world unit. A typical tree is ~500 units
  tall; a champion is ~200–300 units tall.

## Cache reader

Verified details of the pinned OSRS cache:

- `main_file_cache.dat2` is a 520-byte sector store.
- Index files are arrays of 6-byte entries: 3-byte length, 3-byte first sector.
- Sector header: 8 bytes (`group id` u16, `chunk index` u16, `next sector` u24,
  `next chunk` u8).
- Group header: compression u8 + compressed length u32. For encrypted map
  groups, XTEA starts at byte 5 and includes the 4-byte decompressed length.
- Compression: 0 none, 1 bzip2 (stream header stripped; prepend `BZh9`),
  2 gzip (footer is damaged; decompress raw deflate and ignore it), 3 LZMA.
- Index 255 holds the per-index reference tables (protocol 5–7, names, CRCs,
  file counts, file IDs).

The reader exposes `Cache::group`, `Cache::archive_files`, `Cache::file`, and
`Cache::config`. `archive_files` implements RuneLite's chunked archive layout for
multi-file groups.

## Renderer

M0 renderer (in `crates/client`):

- One `Vertex { position: [f32;3], color: [f32;4] }` buffer layout.
- Terrain: 65×65 corner grid per region, height/color averaged from adjacent
  tiles, two triangles per tile.
- Scenery: selected locations are resolved through object configs, models are
  loaded from index 7, transformed to world space, and emitted as flat-shaded
  triangles (one shade per face).
- Shader: camera uniform, distance fog to a sky color, no textures yet.
- Depth: `Depth32Float`, `Less`, no culling until terrain winding is finalized.

Planned renderer work:

- Chunked static meshes (64×64 tiles) with frustum culling.
- Per-face RS light table rather than the current normal dot product.
- Texture atlas and nearest-neighbor sampling; alpha-tested foliage.
- CPU vertex animation first (sequence frames), then baked animation textures
  for minions if profiling requires.
- Water shader, blob shadows, selection rings, team outlines, health bars.
- Fog-of-war overlay and minimap.

## Simulation and server (planned)

- Fixed 30 Hz tick; inputs sampled at 30 Hz; snapshots at 20 Hz.
- Authoritative server validates movement, abilities, cooldowns, mana, range,
  vision, gold, and match state.
- Client predicts own movement and reconciles against server acks. Remote units
  are interpolated ~100 ms behind.
- Fog is server-side: snapshots omit entities the team cannot see.
- See [NETCODE.md](NETCODE.md).

## Data flow for content

1. Cache IDs are the source of truth for visuals: `npc`, `obj`, `loc`, `model`,
   `spotanim`, `seq`.
2. `data/champions/*.json` and `data/items/*.json` reference those IDs and carry
   MOBA stats/abilities.
3. `data/maps/*` carries terrain overrides, lanes, towers, camps, fog blockers,
   and spawns; baked collision/nav/minimap are separate files.
4. The server loads the same data as the client; an asset hash mismatch refuses
   the connection.

## Performance targets

- 1080p / 60 fps on the AMD Radeon 680M iGPU.
- <16.6 ms frame time; <8 ms sim tick for 10 champions + 60 minions + 20 camps.
- <150 MB GPU memory for the v1 map and champion set.
- Headless 10-bot match at 20× speed for balance/soak tests.
