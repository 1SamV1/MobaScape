# Status

Last updated: M3 first lane, 2026-11-13.

## Working

- Rust workspace builds; `cargo test --workspace` and `cargo clippy
  --workspace --all-targets` are clean.
- Cache reader: all 25 indices, reference tables, XTEA, gzip/bzip2/LZMA,
  archive splitting with real file IDs.
- Terrain/locations/object/NPC/underlay/overlay configs.
- Model formats 2 and 3 with flat-shading normals, vertex groups, texture refs.
- Sequence/frame/framemap decoding and interpolated CPU vertex animation.
- `crates/sim`: deterministic 192×192 tile grid, A* with octile heuristic,
  no corner cutting, nearest-walkable fallback, path simplification, LOS.
- Scene: 3×3 Karamja block, 37,249 terrain vertices, 900 scenery placements,
  8 ambient animated NPCs, lane path ribbon, two towers + one nexus per team.
- Movement: click-to-move A* path following; hold-to-steer with axis-separated
  collision sliding; camera follow/pan; selection and click rings.
- Lane: `data/maps/karamja_rift/map.json` defines the mid lane and structures.
  Five-minion waves every 25 s (3 skeletons, 2 skeleton mages) per team.
  Minion lane AI, tower targeting/projectiles, nexus health, and a win condition.
- Combat: player auto-attacks, Q fire blast, hit flash, health bars, dummy
  respawn. Player death/respawn at the spawn point.
- HUD: screen-space health/mana bars, Q cooldown box, crosshair, team nexus
  health bars, wave timer; numeric values in the window title.
- Dev demos: `--demo-target X Y`, `--demo-combat`, `--timescale N`.

## Verified

- `cargo test --workspace`: 13 tests pass (6 cache, 7 sim).
- `cargo clippy --workspace --all-targets`: no warnings.
- `--timescale 20` run: 50 minions, west tower at **198/450**, team 0 nexus at
  **870/1200**, title reports wave timer and nexus health. Tower and nexus
  damage is live.
- `--demo-combat`: dummy target health 58/120 after auto-attacks + Q.
- HUD screenshot check: nexus bars, wave timer, player/dummy/tower health bars,
  Q box, crosshair, lane ribbon, skeleton minions, obelisk towers.

## Known issues

- One mid lane only. No top/bottom lanes, inhibitors, jungle camps, brush, or
  fog. No items, gold, XP, or champion kit beyond Q.
- The lane ribbon can clip under steep terrain in a few spots; it is a display
  overlay, not authored terrain.
- Collision is approximate: water (`underlay_id == 0`), object footprints, and
  3×3 structure footprints. No cliffs, bridges, doors, or authored collision.
- Minion cap is 24 per team; waves can temporarily exceed it by one wave.
- The HUD has no text/font; numbers live in the window title.
- Textures are not sampled; textured faces use flat fallbacks. Dead trees read
  as pale spikes, palms as neon green.
- Animaya is decoded as metadata but not applied.
- Ambient NPCs idle only. No attack animation for champions.
- LZMA is implemented but not exercised by cache 2392.
- Model type 1 and old-format models still error.
- No lobby, netcode, audio, or map editor.

## Next tasks

1. Three lanes, inhibitors, and a proper base/fountain layout.
2. Jungle camps/buffs and gold/XP/items.
3. Champion kit: passive + Q/W/E/R, mana costs, cooldown UI, VFX.
4. HUD text/font atlas and minimap.
5. Deterministic 30 Hz sim tick in `crates/sim` + state hash.
6. Texture sampling and better foliage/trunk colours.
7. Lobby + QUIC netcode after the sim loop is stable.

## Verification log

- `cargo test -p rift-sim`: 7 tests (straight path, wall detour, nearest
  walkable, no corner cutting, simplification, bounds, LOS).
- `cargo test -p rift-cache`: 6 tests (smart reads, chained smart, XTEA, HSL,
  group rejection, framemap/frame parse).
- Lane timescale run: tower 198/450, nexus0 870/1200, 50 minions.
- Scene: 9 regions, 37,249 terrain verts, 398,526 model verts, 900 placements,
  8 ambient NPCs, lane structures.
