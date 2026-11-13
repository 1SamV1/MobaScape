# Status

Last updated: M3.99 champion roster, 2026-11-13.

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
- Map: top/mid/bottom lanes, two towers + one inhibitor per lane per team, one
  nexus per team, four jungle camps, four brush rectangles.
- Movement: click-to-move A* path following; hold-to-steer with collision
  sliding; camera follow/pan; selection and click rings.
- Lane: three-minion waves per lane every 25 s; minion AI, tower targeting,
  inhibitor-gated nexus, victory/defeat.
- Jungle: four neutral camps with respawn; Might (+25% damage) or Swift (+20%
  speed) team buffs for 60 s.
- Champion roster: three data-driven configs in `data/champions/`:
  - **Monkey** — balanced, 500 HP / 200 mana.
  - **JalRek-Jad** — tank/mage, 700 HP / 260 mana, higher damage.
  - **Kree'arra Jr.** — ranged skirmisher, 420 HP / 240 mana, ranged auto-attacks.
  Load with `--champion monkey|jad|kree`.
- Kit: Q projectile, W blink, E heal+shield, R meteor; mana, cooldowns, shield
  absorption, expanding rings.
- Progression: levels 1–11, XP, gold; level-ups raise health, mana, damage.
- Fog/brush: 192×192 visibility grid; hidden enemies/neutrals; dark fog mesh;
  minimap hides unseen units.
- HUD: health/mana/XP/shield/buff bars, four ability boxes, nexus bars, wave
  timer, crosshair, live minimap; numeric state in the title.
- Dev demos: `--demo-target`, `--demo-combat`, `--demo-camp`, `--demo-kit`,
  `--champion`, `--timescale`.

## Verified

- `cargo test --workspace`: 13 tests pass (6 cache, 7 sim).
- `cargo clippy --workspace --all-targets`: no warnings.
- **JalRek-Jad**: title `HP 700/700`, `MP 191/260`, W/E/R cast true.
- **Kree'arra Jr.**: title `HP 420/420`, `MP 170/240`, W/E/R cast true.
- **Monkey** remains the default and still casts W/E/R.
- Fog run: `visible_enemies=7–13` while the rest of the map is dark.
- 20× three-lane run still reaches victory states; jungle Might buff still works.

## Known issues

- Champions share the Q/W/E/R shape; only stats/models/ability numbers differ.
  No champion-select UI, passive, or item shop. Gold is tracked but unspent.
- Kree'arra/Jad models are large; scale tuning is ongoing.
- Fog/brush is client-side only; server-authoritative fog is required for
  multiplayer.
- Fog has hard tile edges and some tall trees/allied vision pockets poke through.
- No minimap fog overlay or click-to-pan.
- Camps are passive and single-model; composite NPCs not assembled.
- No inhibitor respawn/super minions or base/fountain.
- HUD has no text/font; numbers live in the window title.
- Lane ribbons can clip under steep terrain in a few spots.
- Collision is approximate; textures are flat fallbacks; Animaya not applied.
- No lobby, netcode, audio, or map editor.

## Next tasks

1. Item shop + item stats/passives; five champions with distinct kits.
2. Server-authoritative fog + minimap click-to-pan + brush visuals.
3. Inhibitor respawn/super minions and base/fountain.
4. HUD text/font atlas.
5. Deterministic 30 Hz sim tick in `crates/sim` + state hash.
6. Texture sampling and better foliage/trunk colours.
7. Lobby + QUIC netcode after the sim loop is stable.

## Verification log

- `cargo test -p rift-sim`: 7 tests (straight path, wall detour, nearest
  walkable, no corner cutting, simplification, bounds, LOS).
- `cargo test -p rift-cache`: 6 tests (smart reads, chained smart, XTEA, HSL,
  group rejection, framemap/frame parse).
- Champion runs: Jad 700 HP / 260 mana; Kree 420 HP / 240 mana; W/E/R true.
- Fog run: visible enemies 7–13; dark fog mesh over unseen tiles.
- Three-lane 20× run: victory states, towers/inhibitors/nexus live.
- Jungle demo: camp killed, buff Might 58s.
