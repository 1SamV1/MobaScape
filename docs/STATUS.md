# Status

Last updated: M4.2 HUD font + base loop, 2026-11-13.

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
  8 ambient animated NPCs, three stone lane ribbons, two fountain rings.
- Map: top/mid/bottom lanes, two towers + one inhibitor per lane per team, one
  nexus per team, four jungle camps, four brush rectangles.
- Movement: click-to-move A* path following; hold-to-steer with collision
  sliding; camera follow/pan; selection and click rings; minimap click-to-pan.
- Lane: three-minion waves per lane every 25 s; minion AI, tower targeting,
  inhibitor-gated nexus, victory/defeat.
- Inhibitors: destroyed inhibitors respawn after 120 s. While an enemy
  inhibitor is down, that lane's waves include a super minion (2.5x health,
  2x damage, 1.6x scale, gold minimap marker). Respawn re-shields the nexus.
- Jungle: four neutral camps with respawn; Might (+25% damage) or Swift (+20%
  speed) team buffs for 60 s.
- Champion roster: three data-driven configs in `data/champions/`:
  - **Monkey** — balanced, 500 HP / 200 mana.
  - **JalRek-Jad** — tank/mage, 700 HP / 260 mana, higher damage.
  - **Kree'arra Jr.** — ranged skirmisher, 420 HP / 240 mana, ranged auto-attacks.
  Load with `--champion monkey|jad|kree`.
- Kit: Q projectile, W blink, E heal+shield, R meteor; mana, cooldowns, shield
  absorption, expanding rings; ability power from items scales Q/R/E.
- Progression: levels 1–11, XP, gold; level-ups raise health, mana, damage.
- Economy/base: passive gold 8/s; fountain heals 6% max HP/MP/s within 900
  units of spawn; two persistent rings mark the fountain.
- Item shop: six data-driven items in `data/items/items.json` (bronze sword,
  iron platebody, amulet of fury, berserker ring, Ahrim's staff, dragon boots),
  six inventory slots, fountain-range purchase, gold spending, and flat
  damage/health/mana/speed/ability-power stats. `B` toggles the shop, `1`–`6`
  buys, clicking a row buys, clicking outside or `Esc` closes. Hovering a row
  shows a stat tooltip.
- HUD font: built-in 5x7 uppercase bitmap font in `crates/client/src/font.rs`.
  On-screen HP/MP/XP/gold, nexus values, wave timer, ability labels and
  cooldowns, shop names/costs/tooltips, and victory/defeat.
- Fog/brush: 192×192 visibility grid; hidden enemies/neutrals; dark fog mesh;
  minimap draws 8×8-tile fog blocks and hides unseen units.
- HUD: health/mana/XP/shield/buff bars, four ability boxes, six inventory slots,
  shop panel with item-colour swatches/cost bars/owned markers/tooltips, nexus
  bars, wave timer, crosshair, fogged live minimap.
- Dev demos: `--demo-target`, `--demo-combat`, `--demo-camp`, `--demo-kit`,
  `--demo-shop`, `--demo-base`, `--demo-inhibitor`, `--champion`, `--timescale`.

## Verified

- `cargo test --workspace`: 21 tests pass (6 cache, 8 client, 7 sim).
- `cargo clippy --workspace --all-targets`: no warnings.
- **Item shop demo**: granted 4000 gold; bought all six items; final title
  `HP 730/730`, `MP 260/260`, `gold 300`, `dmg 45 spd 460`, `items 6/6`,
  ability power 45%.
- **Shop UI capture**: item names/costs, owned markers, cost bars, inventory
  initials, gold header, hovered `IRON PLATEBODY +150 HP CLICK TO BUY` tooltip.
- **Base demo**: damaged to `200/500` HP; after fountain time `410/500` HP and
  passive gold `8 -> 64`.
- **Inhibitor demo**: destroyed lane-0 inhibitor `inhib1=(2,3)`,
  `vulnerable1=true`; after respawn `state=(700/700, dead_timer=0)`,
  `inhib1=(3,3)`, `vulnerable1=false`, super minions present.
- **Minimap**: fogged 8×8 blocks and click-to-pan coordinate tests pass.
- **JalRek-Jad**: title `HP 700/700`, `MP 191/260`, W/E/R cast true.
- **Kree'arra Jr.**: title `HP 420/420`, `MP 170/240`, W/E/R cast true.
- Fog run: `visible_enemies=7–13` while the rest of the map is dark.
- 20× three-lane run still reaches victory states; jungle Might buff still works.

## Known issues

- Item purchases and gold/heal/shop-range are client-side in this prototype.
  The authoritative server must own economy, inventory, and stat application.
- No item selling, combining, passives, or active item effects; six flat-stat
  items only.
- The HUD font is a code-defined 5x7 bitmap, not a cache-native OSRS font; no
  wrapping, alignment beyond centering, or scoreboard/lobby screens yet.
- Champions share the Q/W/E/R shape; only stats/models/ability numbers differ.
  No champion-select UI or passive.
- Kree'arra/Jad models are large; scale tuning is ongoing.
- Fog/brush is client-side only; server-authoritative fog is required for
  multiplayer. Fog has hard tile edges and the minimap fog is coarse (8×8).
- No minimap brush differentiation or click-drag ping.
- Camps are passive and single-model; composite NPCs not assembled.
- Base/fountain visuals are two rings, not a modeled fountain or shop building.
- Lane ribbons can clip under steep terrain in a few spots.
- Collision is approximate; textures are flat fallbacks; Animaya not applied.
- No lobby, netcode, audio, or map editor.

## Next tasks

1. Item selling/combining/passives and five champions with distinct kits.
2. Cache-native font atlas and a Tab scoreboard/lobby screen.
3. Server-authoritative fog, economy, and shop transactions.
4. Base/fountain model and shop building; brush visuals.
5. Deterministic 30 Hz sim tick in `crates/sim` + state hash.
6. Texture sampling and better foliage/trunk colours.
7. Lobby + QUIC netcode after the sim loop is stable.

## Verification log

- `cargo test -p rift-sim`: 7 tests (straight path, wall detour, nearest
  walkable, no corner cutting, simplification, bounds, LOS).
- `cargo test -p rift-cache`: 6 tests (smart reads, chained smart, XTEA, HSL,
  group rejection, framemap/frame parse).
- `cargo test -p rift-client`: 8 tests (4 items, 2 font, 2 minimap).
- Shop demo: 6/6 items, dmg 45, HP 730/730, MP 260/260, speed 460, AP 45%,
  gold 300.
- Base demo: 200/500 -> 410/500 HP; gold 8 -> 64.
- Inhibitor demo: 2/3 -> respawn 3/3; nexus re-shielded; super minions spawned.
- Champion runs: Jad 700 HP / 260 mana; Kree 420 HP / 240 mana; W/E/R true.
- Fog run: visible enemies 7–13; dark fog mesh over unseen tiles.
- Three-lane 20× run: victory states, towers/inhibitors/nexus live.
- Jungle demo: camp killed, buff Might 58s.
