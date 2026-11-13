# Status

Last updated: M3.9 champion kit, 2026-11-13.

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
  two towers + one inhibitor per lane per team, one nexus per team, and four
  jungle camps.
- Movement: click-to-move A* path following; hold-to-steer with axis-separated
  collision sliding; camera follow/pan; selection and click rings.
- Lane: three-minion waves per lane every 25 s (two skeletons, one skeleton
  mage) per team. Minion lane AI, tower targeting/projectiles, inhibitor-gated
  nexus, victory/defeat.
- Jungle: four neutral camps (giant, spider, bat, abyssal demon) with respawn
  timers. Killing a camp grants the attacker's team Might (+25% damage) or
  Swift (+20% speed) for 60 s.
- Champion kit: Q fire blast (projectile), W blink (collision-checked),
  E heal + shield, R meteor AoE. Mana costs, cooldowns, shield absorption,
  expanding ring effects.
- Progression: levels 1–11, XP thresholds, gold from kills; level-ups raise
  health, mana, and attack damage.
- Combat: player auto-attacks, hit flash, health bars, dummy respawn. Player
  death/respawn at the spawn point. Towers/minions/camps can damage the player.
- HUD: health/mana/XP bars, shield bar, buff bar, four ability boxes with
  cooldown fills, team nexus bars, wave timer, crosshair, live minimap.
  Numeric values live in the window title.
- Dev demos: `--demo-target X Y`, `--demo-combat`, `--demo-camp`, `--demo-kit`,
  `--timescale N`.

## Verified

- `cargo test --workspace`: 13 tests pass (6 cache, 7 sim).
- `cargo clippy --workspace --all-targets`: no warnings.
- `--demo-kit --timescale 4`: W, E, and R all cast successfully; title reported
  **L1 50/100 XP**, **MP 129/200**, **gold 20**, and
  **Q/W/E/R 0.0/0.0/5.9/42.9s**.
- 20× three-lane run ended in **VICTORY**:
  - team 1 nexus **0/1500 (open)**, team 0 nexus **1500/1500 (shielded)**
  - team 1 inhibitors **2/3**, team 0 **3/3**
  - **72 minions**, several towers destroyed, player HP **428/500**
- Jungle demo: camp killed, title reported **buff Might 58s**, buff bar rendered.
- Minimap screenshot: three lane paths, blue/red structures, orange camps,
  minion dots, player dot.

## Known issues

- No brush or fog of war; the minimap is display-only (not clickable).
- No item shop; gold is tracked but unspent. Only one champion kit (the monkey).
- Camps are passive (they do not retaliate) and single-model; composite NPCs
  are not assembled.
- Three lanes, but no inhibitor respawn/super minions or base/fountain.
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

1. Brush + fog of war, and minimap click-to-pan.
2. Item shop, item stats/passives, and five champions with distinct kits.
3. Inhibitor respawn/super minions and a proper base/fountain.
4. HUD text/font atlas.
5. Deterministic 30 Hz sim tick in `crates/sim` + state hash.
6. Texture sampling and better foliage/trunk colours.
7. Lobby + QUIC netcode after the sim loop is stable.

## Verification log

- `cargo test -p rift-sim`: 7 tests (straight path, wall detour, nearest
  walkable, no corner cutting, simplification, bounds, LOS).
- `cargo test -p rift-cache`: 6 tests (smart reads, chained smart, XTEA, HSL,
  group rejection, framemap/frame parse).
- Champion kit demo: W/E/R cast true; level 1, XP 50/100, gold 20, mana 129/200.
- Three-lane 20× run: victory, nexus 0/1500 vs 1500/1500, inhibitors 2/3 vs 3/3,
  72 minions, player 428/500.
- Jungle demo: camp killed, buff Might 58s.
