# Status

Last updated: M3.5 three-lane map, 2026-11-13.

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
  8 ambient animated NPCs, three stone lane ribbons.
- Map: `data/maps/karamja_rift/map.json` defines top/mid/bottom lanes,
  two towers + one inhibitor per lane per team, and one nexus per team.
- Movement: click-to-move A* path following; hold-to-steer with axis-separated
  collision sliding; camera follow/pan; selection and click rings.
- Lane: three-minion waves per lane every 25 s (two skeletons, one skeleton
  mage) per team. Minion lane AI, tower targeting/projectiles, inhibitor-gated
  nexus, victory/defeat.
- Combat: player auto-attacks, Q fire blast, hit flash, health bars, dummy
  respawn. Player death/respawn at the spawn point. Towers/minions can damage
  the player.
- HUD: screen-space health/mana bars, Q cooldown box, crosshair, team nexus
  health bars, wave timer; numeric values in the window title. Victory/defeat
  screen border.
- Dev demos: `--demo-target X Y`, `--demo-combat`, `--timescale N`.

## Verified

- `cargo test --workspace`: 13 tests pass (6 cache, 7 sim).
- `cargo clippy --workspace --all-targets`: no warnings.
- 20× three-lane run ended in **VICTORY**:
  - team 1 nexus **0/1500 (open)**, team 0 nexus **1500/1500 (shielded)**
  - team 1 inhibitors **2/3**, team 0 inhibitors **3/3**
  - **72 minions**, 6 towers per team, several towers destroyed
  - player HP dropped to **428/500** from tower/minion damage
- `--demo-combat`: dummy target health 58/120 after auto-attacks + Q.
- HUD/lane screenshot: three lane ribbons, nexus crystals, obelisk towers,
  skeleton minions, nexus/wave bars, victory border.

## Known issues

- Three lanes but no jungle camps, brush, fog, inhibitors respawn, or super
  minions.
- No gold/XP/items or full champion kit beyond Q.
- The HUD has no text/font; numbers live in the window title.
- Lane ribbons can clip under steep terrain in a few spots.
- Collision is approximate: water, object footprints, 3×3 structure footprints.
  No cliffs, bridges, doors, or authored collision.
- Minion cap is 36 per team; waves can temporarily exceed it.
- Textures are not sampled; textured faces use flat fallbacks.
- Animaya is decoded as metadata but not applied.
- Ambient NPCs idle only. No attack animation for champions.
- LZMA is implemented but not exercised by cache 2392.
- Model type 1 and old-format models still error.
- No lobby, netcode, audio, or map editor.

## Next tasks

1. Jungle camps/buffs, brush, and fog of war.
2. Gold/XP/items and a champion kit (passive + Q/W/E/R).
3. Inhibitor respawn / super minions and a proper base/fountain.
4. HUD text/font atlas and minimap.
5. Deterministic 30 Hz sim tick in `crates/sim` + state hash.
6. Texture sampling and better foliage/trunk colours.
7. Lobby + QUIC netcode after the sim loop is stable.

## Verification log

- `cargo test -p rift-sim`: 7 tests (straight path, wall detour, nearest
  walkable, no corner cutting, simplification, bounds, LOS).
- `cargo test -p rift-cache`: 6 tests (smart reads, chained smart, XTEA, HSL,
  group rejection, framemap/frame parse).
- Three-lane 20× run: victory, nexus 0/1500 vs 1500/1500, inhibitors 2/3 vs 3/3,
  72 minions, player 428/500.
