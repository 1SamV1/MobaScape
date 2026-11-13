# Status

Last updated: M5.10 collision consistency, 2026-11-13.

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
- Scene: 5×5 Karamja block (radius 2), 614k terrain verts, ~1500 scenery
  placements, 8 ambient animated NPCs, three dirt lane ribbons, distant
  backdrop plane.
- Biome pockets: eight real OSRS regions transplanted into the map (Wilderness
  dead trees, Morytania swamp, ogre ruins, goblin camp on the dire side;
  Varrock, human keep, Tirannwn elven woods, gnome stronghold on the radiant
  side), each with its own ground tint and scenery.
- River: a diagonal waterway from the upper left third through the centre to
  the lower right third, carved into the terrain, blocked for movement except
  at the centre bridge.
- Lane tree lines: two rows of OSRS tree models flank every lane, tinted by
  the side of the river they stand on.
- Jungle: eight neutral camps with proximity aggro, chase, leash reset and
  respawn; Might (+25% damage) or Swift (+20% speed) team buffs for 60 s.
- Presentation: HDR `Rgba16Float` scene target with a real bloom chain
  (bright pass, separable Gaussian, ACES tonemap), cache-native ground
  texture array with world-space UVs, two-sided lighting with a camera fill,
  and a fog of war that shadows unexplored ground instead of tinting it.
- Fog of war: a shader-side horizontal distance circle around the hero
  (1500 units, 0.80R..1.30R soft edge, 42% brightness outside). No fog mesh,
  so it never fights with terrain z levels and the boundary is a consistent
  lateral radius at every height.
- HUD: OSRS palette text at 1.32x, Dota scoreboard strip (kills, deaths,
  clock), cursor tooltip for units, minimap viewport box, biome label.
- Minimap: a baked 256x256 terrain map (biomes, river and lanes) uploaded to
  the UI atlas, team hero markers with a hero profile card (portrait, name,
  level, HP/MP), structure dots for both teams, and a camera viewport box.
  Enemy heroes are never shown; enemy mobs only while inside team LOS.
- Lanes: baked into the terrain as a mask in the vertex alpha; the shader
  blends the OSRS dirt/path sprites over the ground, so there is no ribbon
  overlay, no z-fighting and the road edges wobble naturally.
- Watchtowers: six OSRS "Battle tower" structures (three per team) with a
  620 unit acquire radius and a 1200 unit lock range, firing slow 280 u/s
  bolts at heroes, minions and jungle camps until the target dies or leaves
  the lock range.
- Castle wall ruins: broken OSRS walls and ruin clusters line the outside of
  every lane.
- Scattering: trees and ruins are placed by a fixed-seed xorshift RNG with
  variable spacing, offsets, scales, species and clearings, so the world is
  varied but identical every match. Dire species are dead and dark, radiant
  species stay green. Pocket props are kept 6.5 tiles clear of every lane.
- Visibility: structures (towers, inhibitors, nexus, watchtowers) are always
  visible; projectiles are only drawn inside the team's line of sight.
- Movement: click-to-move A* path following; hold-to-steer with collision
  sliding; camera follow/pan; selection and click rings; minimap click-to-pan.
- Collision: only rendered props block movement (base scenery, pocket props,
  lane trees and ruins), so there are no invisible walls; the pathfinder snaps
  start/goal up to 20 tiles so a hero pushed onto a blocked tile can escape.
- Shop: purchases work anywhere (the fountain still heals and respawns).
- Lane: three-minion waves per lane every 25 s; minion AI, tower targeting,
  inhibitor-gated nexus, victory/defeat.
- Inhibitors: destroyed inhibitors respawn after 120 s. While an enemy
  inhibitor is down, that lane's waves include a super minion (2.5x health,
  2x damage, 1.6x scale, gold minimap marker). Respawn re-shields the nexus.
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
- Item shop: six data-driven items, six inventory slots, fountain-range
  purchase, gold spending, stat/ability-power application, hover tooltips.
- **Terrain smoothing/detail (M5.3a)**: the per-tile color field is box-blurred
  twice and assigned to shared grid-corner vertices; `scene.wgsl` adds
  procedural ground detail for terrain. Tile outlines are gone.
- **Post pass (M5.4a)**: scene renders to an offscreen sRGB target; `post.wgsl`
  applies FXAA, cheap bloom, gamma-space color grading, and dithering. HUD
  renders after post, so text/icons stay crisp.
- **OSRS-style UI**: centered Fountain Shop window with category tabs, item-card
  grid, detail pane, Buy/Sell buttons, inventory row, gold, close button, and
  real item icons/tab sprites from the gitignored `assets/osrs-ui` tree. HUD
  resource bar, team panel, objectives panel, event log, compact health/mana
  bars, bottom-right inventory, and shop button. Textured `ui.wgsl` pipeline
  with a 1024×1024 atlas; procedural fallback if assets are missing.
- HUD font: built-in 5x7 uppercase bitmap font; HP/MP/XP/gold, nexus values,
  wave timer, ability labels/cooldowns, shop names/costs/tooltips, result.
- **117HD Core renderer**:
  - `SceneVertex` carries position, normal, colour, UV, AO, and material.
  - sRGB→linear conversion on the CPU; sRGB target handles final encode.
  - Smooth terrain normals; flat per-face model and unit normals.
  - Sun + hemispheric ambient lighting, optional 4-band stepping.
  - Sky gradient, procedural clouds, sun disk/glow, horizon haze.
  - Exponential height fog and ground haze.
  - Static scenery blob shadows + per-unit blob shadows.
  - Up to 16 dynamic point lights: fountain, Q muzzle/impact, E/R effects,
    projectiles.
  - **Sun shadow map**: 1024px depth map, 60x60-tile orthographic frustum, PCF
    via comparison sampler, dynamic player/structure/camp casters, alternating
    frame updates. `--shadows off|blob|map`.
  - **Contact AO**: baked terrain cavity AO and model/unit base AO.
  - `--graphics 117hd|classic`,
    `--debug-view albedo|normal|lighting|fog|lights|shadow|ao`, `--pitch`.
- Fog/brush: 192×192 visibility grid; hidden enemies/neutrals; dark fog mesh;
  minimap draws 8×8-tile fog blocks and hides unseen units.
- HUD: health/mana/XP/shield/buff bars, four ability boxes, six inventory slots,
  shop panel, nexus bars, wave timer, crosshair, fogged live minimap.
- Dev demos: `--demo-target`, `--demo-combat`, `--demo-camp`, `--demo-kit`,
  `--demo-shop`, `--demo-base`, `--demo-inhibitor`, `--champion`, `--timescale`.

## Verified

- `cargo test --workspace`: 28 tests pass (6 cache, 15 client, 7 sim).
- `cargo clippy --workspace --all-targets`: no warnings.
- 117HD Core runs at **60–68 fps at 1280×720** on the dev machine at timescale 1;
  shadow modes measured **67 fps (off)**, **68 fps (blob)**, **66 fps (map)**.
- Shadow debug view confirmed dark cast shadows under structures/units; AO
  debug view confirmed baked terrain/model contact darkening.
- A/B captures: `--graphics classic` flat/unlit baseline vs `--graphics 117hd`
  lit terrain/models, sky/clouds, fog, blob shadows, dynamic lights.
- Debug views confirmed albedo, normals, lighting, fog, and dynamic-light
  passes; Q impact shows an orange light pool next to the green fountain pool.
- Shop demo: 6/6 items, `dmg 45`, `HP 730/730`, `MP 260/260`, `spd 460`,
  `AP 45%`, `gold 300`.
- Base demo: 200/500 → 410/500 HP; passive gold 8 → 64.
- Inhibitor demo: 2/3 → respawn 3/3; nexus re-shielded; super minions spawned.
- Champion runs: Jad 700 HP / 260 mana; Kree 420 HP / 240 mana; W/E/R true.
- Fog run: visible enemies 7–13; dark fog mesh over unseen tiles.
- Three-lane 20× run: victory states, towers/inhibitors/nexus live.
- Jungle demo: camp killed, buff Might 58s.

## Known issues

- 117HD Core has **no cache-native textures, HD water, MSAA, full SSAO, or
  draw-distance streaming** yet. M5.3a/M5.4a ship terrain smoothing, procedural
  ground detail, FXAA, cheap bloom, color grading, and dithering.
- The post bloom is a cheap 8-tap screen-space glow, not a full HDR bloom chain.
- Draw-distance streaming/LOD is still pending; the map remains a 3×3 block.
- The shadow map covers dynamic units/structures/camps only; static scenery
  still uses blob shadows. Static shadow caching/chunked casters are pending.
- The shadow map is 1024px and updates every other frame; fast camera movement
  can show slight shadow lag.
- The 3×3 region block edge is still visible at low camera pitch. A distant
  backdrop plane and horizon haze hide it from the default camera, but true
  117HD draw distance needs chunked streaming/LOD.
- The fog-of-war mesh is an opaque dark plane. At low pitch it can read as a
  hard wall; it needs a soft/dithered screen-space treatment.
- Lighting is tuned for Karamja afternoon sun; other maps still need their own
  atmosphere presets.
- Dynamic lights are capped at 16 and have no shadows.
- Item purchases and gold/heal/shop-range remain client-side.
- No item selling/combining/passives; champions share the QWER shape.
- The HUD font is a code-defined 5x7 bitmap, not a cache-native OSRS font.
- UI assets live in gitignored `assets/osrs-ui`; without them the shop falls back
  to procedural icons and no tab sprites. The bronze sword icon is a trimmed
  detail render because the plain inventory icon file was unavailable under the
  expected wiki name. Faint baked tab numbers/icons can show through overlays.
- Kree'arra/Jad models are large; scale tuning is ongoing.
- Fog/brush is client-side only; server-authoritative fog is required.
- No lobby, netcode, audio, or map editor.

## Next tasks

1. **M5.2 117HD Water**: water mesh, fresnel, sky reflection, specular, caustics.
2. **M5.3b cache textures**: texture decoding, atlas, model/ground textures,
   ground blending, anisotropic filtering.
3. **M5.4b post + draw distance**: full SSAO, MSAA, proper HDR bloom,
   chunked streaming/LOD, model batching.
4. Static scenery shadow caching/chunked casters.
5. Item selling/combining/passives and five champions with distinct kits.
6. Server-authoritative fog, economy, and shop transactions.
7. Deterministic 30 Hz sim tick in `crates/sim` + state hash.

## Verification log

- `cargo test -p rift-sim`: 7 tests (straight path, wall detour, nearest
  walkable, no corner cutting, simplification, bounds, LOS).
- `cargo test -p rift-cache`: 6 tests (smart reads, chained smart, XTEA, HSL,
  group rejection, framemap/frame parse).
- `cargo test -p rift-client`: 15 tests (5 items/layout, 2 font, 2 minimap,
  4 scene/lighting, 2 shadow matrix/defaults).
- Shop demo: 6/6 items, dmg 45, HP 730/730, MP 260/260, speed 460, AP 45%,
  gold 300.
- Base demo: 200/500 -> 410/500 HP; gold 8 -> 64.
- Inhibitor demo: 2/3 -> respawn 3/3; nexus re-shielded; super minions spawned.
- 117HD debug: albedo/normal/lighting/fog views render; dynamic lights show
  green fountain and orange Q impact pools; shadow/AO debug views render.
- Shadow modes: off 67 fps, blob 68 fps, map 66 fps at 1280×720.
- UI run: real item icons/tab sprites render; shop open at 58–60 fps.
- Ground: A/B FOW-disabled capture is seamless; continuous fog mesh removes
  per-tile grid lines.
- Post: FXAA/grade/bloom at 59–60 fps; HUD crisp after post.
- Champion runs: Jad 700 HP / 260 mana; Kree 420 HP / 240 mana; W/E/R true.
- Fog run: visible enemies 7–13; dark fog mesh over unseen tiles.
- Three-lane 20× run: victory states, towers/inhibitors/nexus live.
- Jungle demo: camp killed, buff Might 58s.
