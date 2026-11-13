# Status

Last updated: M3.95 fog + brush, 2026-11-13.

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
  two towers + one inhibitor per lane per team, one nexus per team, four
  jungle camps, and four brush rectangles.
- Movement: click-to-move A* path following; hold-to-steer with axis-separated
  collision sliding; camera follow/pan; selection and click rings.
- Lane: three-minion waves per lane every 25 s (two skeletons, one skeleton
  mage) per team. Minion lane AI, tower targeting/projectiles, inhibitor-gated
  nexus, victory/defeat.
- Jungle: four neutral camps (giant, spider, bat, abyssal demon) with respawn
  timers. Killing a camp grants the attacker's team Might (+25% damage) or
  Swift (+20% speed) for 60 s.
- Champion kit: Q fire blast, W blink, E heal + shield, R meteor AoE. Mana
  costs, cooldowns, shield absorption, expanding ring effects.
- Progression: levels 1–11, XP thresholds, gold from kills; level-ups raise
  health, mana, and attack damage.
- Fog/brush: 192×192 visibility grid from allied units/structures; enemy and
  neutral units outside vision are hidden; brush zones hide units unless the
  player is within 500 units; an opaque dark fog mesh covers unseen tiles.
  Minimap hides unseen units.
- HUD: health/mana/XP bars, shield bar, buff bar, four ability boxes with
  cooldown fills, team nexus bars, wave timer, crosshair, live minimap.
  Numeric values live in the window title.
- Dev demos: `--demo-target X Y`, `--demo-combat`, `--demo-camp`, `--demo-kit`,
  `--timescale N`.

## Verified

- `cargo test --workspace`: 13 tests pass (6 cache, 7 sim).
- `cargo clippy --workspace --all-targets`: no warnings.
- Fog run: `visible_enemies=7` to `13` while the rest of the map is dark;
  minimap hides unseen units; player/team units remain visible.
- Champion kit demo: W/E/R cast true; title **L4 80/340 XP**, **MP 260/260**,
  **gold 310**, **Q/W/E/R 0.0/0.0/0.0/0.0s**.
- 20× three-lane run still reaches victory states with towers/inhibitors/nexus.
- Jungle demo: camp killed, **buff Might 58s**, buff bar rendered.

## Known issues

- Fog/brush is client-side only; server-authoritative fog is a netcode milestone.
- Fog has hard tile edges and some tall trees/allied vision patches poke through
  the dark plane. No minimap fog overlay or click-to-pan yet.
- No item shop; gold is tracked but unspent. Only one champion kit (monkey).
- Camps are passive (no retaliation) and single-model; composite NPCs not
  assembled.
- No inhibitor respawn/super minions or base/fountain.
- HUD has no text/font; numbers live in the window title.
- Lane ribbons can clip under steep terrain in a few spots.
- Collision is approximate: water, object footprints, 3×3 structure footprints.
- Minion cap is 36 per team; waves can temporarily exceed it.
- Textures are not sampled; textured faces use flat fallbacks.
- Animaya is decoded as metadata but not applied.
- Ambient NPCs idle only. No attack animation for champions.
- LZMA is implemented but not exercised by cache 2392.
- Model type 1 and old-format models still error.
- No lobby, netcode, audio, or map editor.

## Next tasks

1. Server-authoritative fog and minimap click-to-pan/brush visuals.
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
- Fog run: visible enemies 7–13; dark fog mesh over unseen tiles.
- Champion kit demo: W/E/R cast true; level 4, XP 80/340, gold 310, mana 260/260.
- Three-lane 20× run: victory states, towers/inhibitors/nexus live.
- Jungle demo: camp killed, buff Might 58s.
