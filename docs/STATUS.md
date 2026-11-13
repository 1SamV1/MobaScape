# Status

Last updated: M2 combat sandbox, 2026-11-13.

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
- Scene: 3×3 Karamja block (192×192 tiles), 37,249 terrain vertices, 900
  scenery placements, 8 animated NPCs (monkeys/snakelings).
- Movement: click-to-move A* path following; hold-to-steer with axis-separated
  collision sliding; camera follow/pan; selection and click rings.
- Combat sandbox: three respawning target dummies (120 HP), auto-attacks
  (12 damage / 1.15 s, range 330 units), Q fire blast (38 damage, 5 s cooldown,
  projectile), hit flash, death/respawn.
- HUD: screen-space health/mana bars, Q cooldown box, crosshair; numeric values
  in the window title (`HP`, `Q`).
- `rift-tools`: `cache-info`, `terrain`, `locations`, `object`, `find-object`,
  `npc`, `find-npc`, `sequence`, `animate`, `model`, `export-obj`,
  `terrain-obj`, `underlay`, `overlay`, `scan-colors`.
- Dev demos: `--demo-target X Y` (walk to a tile) and `--demo-combat`
  (target a dummy, auto-attack, fire Q).

## Verified

- `cargo test --workspace`: 13 tests pass (6 cache, 7 sim).
- `cargo clippy --workspace --all-targets`: no warnings.
- Pathfinding: `--demo-target 2900 3060` logged `3 waypoints` and the champion
  moved from tile ~2847,3038 toward the target.
- Combat: `--demo-combat` logged target 11, Q cast true, and target health 58
  after auto-attacks + Q (from 120).
- HUD screenshot check: health/mana bars, Q box, crosshair, dummy health bars,
  and the orange projectile are visible.

## Known issues

- Collision is approximate: water (`underlay_id == 0`) and conservative object
  footprints. No cliffs, bridges, doors, or authored collision yet.
- The 3×3 world is fixed; no terrain skirt or streaming.
- Textures are not sampled. `0xff00ff` overlay sentinels fall back to underlay
  colours; textured model faces use a flat green fallback. Dead trees read as
  pale spikes and palms as neon green.
- Animaya is decoded as metadata but not applied.
- HUD has no text/font yet; numeric values live in the window title.
- Combat has no mana cost, no champion kit beyond Q, no minions/towers/items,
  no fog, no lobby, no netcode, no audio, no map editor.
- NPCs idle only; the player plays idle/walk clips. No attack animation.
- LZMA is implemented but not exercised by cache 2392.
- Model type 1 and old-format models still error.

## Next tasks

1. Carve authored lanes + tower/nexus pads; bake authored collision/nav.
2. Minion waves and tower targeting; a real win condition.
3. Champion kit: passive + Q/W/E/R, mana costs, cooldown UI, VFX.
4. HUD text/font atlas and a minimap.
5. Deterministic 30 Hz sim tick in `crates/sim` + state hash.
6. Texture sampling and better foliage/trunk colours.
7. Lobby + QUIC netcode (after the sim loop is stable).

## Verification log

- `cargo test -p rift-sim`: 7 tests (straight path, wall detour, nearest
  walkable, no corner cutting, simplification, bounds, LOS).
- `cargo test -p rift-cache`: 6 tests (smart reads, chained smart, XTEA, HSL,
  group rejection, framemap/frame parse).
- Scene: 9 regions, 37,249 terrain verts, 389,181 model verts, 900 placements,
  8 NPCs, 9 ambient animated units + 3 dummies.
- Demo combat target health 58/120 after auto-attacks + Q.
