# Status

Last updated: M1 vertical slice, 2026-11-13.

## Working

- Rust workspace builds; `cargo test --workspace` and `cargo clippy
  --workspace --all-targets` are clean.
- Cache reader: all 25 indices, reference tables, XTEA, gzip/bzip2/LZMA,
  archive splitting with real file IDs.
- Terrain/locations/object/NPC/underlay/overlay configs.
- Model formats 2 and 3 with flat-shading normals, vertex groups, texture refs.
- Sequence/frame/framemap decoding and interpolated CPU vertex animation.
- `rift-tools`: `cache-info`, `terrain`, `locations`, `object`, `find-object`,
  `npc`, `find-npc`, `sequence`, `animate`, `model`, `export-obj`,
  `terrain-obj`, `underlay`, `overlay`, `scan-colors`.
- Client: 3×3 Karamja region block (192×192 tiles), 37,249 terrain vertices,
  900 scenery placements, 8 animated NPCs (monkeys/snakelings), a click-to-move
  monkey champion, selection ring, click marker, camera follow, FPS/tile title.
- Verified animation:
  - NPC 23 "Monkey", model 7744, idle sequence 222 (2 frames, 21 ticks each),
    walk sequence 219 (5 frames, 4/5/4/4/3 ticks).
  - `animate 7744 222` and `animate 7744 219` show changing bounds; the viewer
    plays both.
- Verified input: a synthetic LMB click on the terrain produced `MOVE` targets
  and the champion walked there.

## Known issues

- Movement is direct steering; no A*, collision, water/cliff checks, or object
  footprints. The champion can walk through scenery and water.
- Textures are not sampled. `0xff00ff` overlay sentinels fall back to underlay
  colours; textured model faces use a flat green fallback. Dead trees read as
  pale spikes and palms as neon green.
- Animaya is decoded only as a flag; it is not applied.
- NPCs are idle; only the player plays idle/walk clips.
- No HUD, health bars, abilities, combat, minions, towers, fog, lobby, netcode,
  audio, or map editor.
- The world is a fixed 3×3 block; no terrain skirt/streaming.
- Camera follow overwrites pan until `WASD` is pressed; `F` toggles follow.
- Plain mouse wheel is ignored; zoom is `Ctrl`+wheel to avoid accidental zoom.
- LZMA is implemented but not exercised by cache 2392.
- Model type 1 and old-format models still error.

## Reproduce

```sh
cargo run -p rift-tools -- find-npc monkey --limit 5
cargo run -p rift-tools -- sequence 222
cargo run -p rift-tools -- animate 7744 222
cargo run -p rift-client
```

Viewer controls: LMB move, WASD pan, Q/E rotate, Ctrl+wheel zoom, F follow,
R reset, Esc quit.

## Next tasks

1. A* pathfinding + collision on the tile grid, shared by client and sim.
2. Carve the first MOBA lanes and place tower/nexus pads; bake collision/nav.
3. Minimal HUD: health/mana, ability bar, FPS/coords, minimap.
4. A real champion kit (Q/W/E/R) with one ability and auto-attacks.
5. Sprite/font extraction for HUD and item icons.
6. Terrain texture sampling and proper foliage/trunk colours.
7. Deterministic `crates/sim` skeleton + state hash.

## Verification log

- `cargo test -p rift-cache`: 6 tests pass (smart reads, XTEA round trip, HSL,
  group rejection, framemap/frame parse).
- `cargo clippy --workspace --all-targets`: no warnings.
- Scene: 9 regions, 37,249 terrain verts, 389,181 model verts, 900 placements,
  8 NPCs, 9 animated units (8 NPCs + player).
- Click test: `MOVE target=...` logged and champion moved; camera followed.
