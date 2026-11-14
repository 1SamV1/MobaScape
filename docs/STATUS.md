# Status

Last updated: M5.23 benchmark harness, shadow proxies + terrain fetch pass, 2026-11-14.

## Working

- Rust workspace builds; `cargo test --workspace` and `cargo clippy
  --workspace --all-targets` are clean.
- Cache reader: all 25 indices, reference tables, XTEA, gzip/bzip2/LZMA,
  archive splitting with real file IDs.
- Terrain/locations/object/NPC/underlay/overlay configs.
- Model formats 2 and 3 with flat-shading normals, vertex groups, texture refs.
- Cache texture pipeline: index 9 texture definitions resolve to index 8 sprite
  archives; 208 sprites decode to a mipmapped 256-layer texture array.
- Sequence/frame/framemap decoding and interpolated CPU vertex animation.
- `crates/sim`: deterministic 192×192 tile grid, A* with octile heuristic,
  no corner cutting, nearest-walkable fallback, path simplification, LOS.
- Scene: 5×5 Karamja block (radius 2), 614k terrain verts, ~1500 scenery
  placements, 1200 cache-native ground decorations (flowers, ferns, grass,
  mushrooms, rocks, twigs, roots and stumps), 8 ambient animated NPCs, three
  dirt lane ribbons, distant backdrop plane.
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
  Camps are revealed by proximity (1500 units) regardless of brush or tree
  line of sight, so they never ambush the player from a hidden tile. The eight
  scattered critters are weak camps too (220 HP, 14 damage, 420 aggro). The two
  camps that accidentally used a table prop model now use creature models
  (General Graardor Jr. 27660 and Zilyana Jr. 27989).
- Ground textures: lush green-boosted grass, stable world-space planar UVs,
  contact darkening instead of a projected cliff overlay, and two noise scales
  for natural break-up. Lane roads use cache soil textures and are suppressed
  under the river. Dead and snowy trees use bark on every face.
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
- Scattering: trees, ruins and ground decor are placed by a fixed-seed
  xorshift RNG with variable spacing, offsets, scales, species and clearings,
  so the world is varied but identical every match. Ground decorations are
  selected from real OSRS object locations and sorted by lane proximity so
  the detail follows the play area. Dire species are dead and dark, radiant
  species stay green. Pocket props are kept 6.5 tiles clear of every lane.
- Visibility: structures (towers, inhibitors, nexus, watchtowers) are always
  visible; projectiles are only drawn inside the team's line of sight.
- Movement: click-to-move A* path following; hold-to-steer with collision
  sliding; camera follow/pan; selection and click rings; minimap click-to-pan.
- Collision: only rendered props block movement (base scenery, pocket props,
  lane trees and ruins), so there are no invisible walls; the pathfinder snaps
  start/goal up to 20 tiles so a hero pushed onto a blocked tile can escape.
- Selection/camp/structure rings sample the height grid per vertex, so the
  bands follow slopes instead of clipping through them. Scenery props keep six
  tiles clear of every camp and structure.
- Shop: purchases work anywhere (the fountain still heals and respawns).
  Remote purchases go to the storage chest beside the team spawn; a HUD button
  dispatches the flying Armadylean guard, which delivers the stored items to
  the hero and returns to its post. The shop marks waiting items `IN TRANSIT`.
- Delivery: chest model 1249 with a gold ring while loaded; guard model 28019
  (Kree'arra Jr., the closest Armadylean model in this cache) at 1100 u/s.
  `--demo-delivery` verifies buy-away-from-base -> dispatch -> handover.
- Lane: three-minion waves per lane every 25 s, now composed of cache-native
  imps (melee) and fairies (ranged), spawned in a lane-relative wedge with
  parallel lane tracks and staggered spawn delays. Small team rings and
  wider health bars read the two sides at a glance.
- Inhibitors: destroyed inhibitors respawn after 120 s. While an enemy
  inhibitor is down, that lane's waves include a super minion (2.5x health,
  2x damage, 1.6x scale, gold minimap marker). Respawn re-shields the nexus.
- Champion roster: three data-driven configs in `data/champions/`:
  - **Monkey** — balanced, 500 HP / 200 mana.
  - **JalRek-Jad** — tank/mage, 700 HP / 260 mana, higher damage.
  - **Kree'arra Jr.** — ranged skirmisher, 420 HP / 240 mana, ranged auto-attacks.
  Load with `--champion monkey|jad|kree`. A normal launch opens the hero-select
  screen; `--champion` skips it for demos and direct matches.
- **Hero select (M5.19)**: full-screen OSRS-style champion picker with baked
  cache-model portraits (CPU rasteriser, idle pose), role/tagline cards, stat
  bars, click/keyboard selection, and a gold lock-in button. Selecting a hero
  rebuilds a fresh match scene so skills, stats and cooldowns start clean.
- **Nexus/inhibitor assets**: the painted blue crystal is replaced by real
  cache models — Portal Nexus (36053) for both nexuses, Crystal of Power
  (12953) for radiant inhibitors, and Crystal Outcrop (9721) for dire
  inhibitors, with team tinting.
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
  - Static scenery blob shadows + per-unit blob shadows, plus a static
    sun-shadow caster mesh for trees, ruins and pocket buildings.
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

- `cargo test --workspace`: 40 tests pass (9 cache, 24 client, 7 sim).
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
- Model textures: 208 cache texture sprites uploaded to a 256-layer array;
  textured faces sample native leaf/wood/stone/flower art instead of flat
  fallbacks, with wood tree bases below the leaf canopy.
- Water: 9,712-triangle river surface with two animated ripple layers,
  fresnel sky reflection, sun specular and fog. Water is walkable and no longer
  an invisible collision wall.
- MSAA: the HDR scene target is 4x multisampled and resolves into the post/bloom
  chain; HUD remains single-sample and crisp.
- Static shadows: trees, ruins and pocket buildings cast into the 1024px sun
  shadow map alongside units.
- Combat presentation: jungle camp health slashed, critter camps are 150 HP /
  9 damage, camp bars are red and wider, and unit bars are pulled toward the
  camera to stop depth flicker while moving.
- Lane road: dirt-dominant, noise-warped 310-unit UVs remove the repeating
  cobble border while keeping the cache path sprite as a low-weight accent.
- Chunk culling: 400 terrain + 212 model + 49 water chunks. Release 1280×720
  immediate benchmark: p50 13.24 ms / p95 14.89 ms with shadows + 4× MSAA,
  p50 12.93 ms / p95 14.20 ms with shadows off, p50 12.17 ms / p95 13.88 ms
  with 1× MSAA. The `--benchmark <seconds>` harness prints these percentiles.
- Fog run: visible enemies 7–13; dark fog mesh over unseen tiles.
- Three-lane 20× run: victory states, towers/inhibitors/nexus live.
- Jungle demo: camp killed, buff Might 58s.

## Known issues

- 117HD Core now has cache model textures, an HD river surface, 4x MSAA and
  static sun casters. It still has **no full SSAO, anisotropic filtering, or
  draw-distance streaming**. Terrain still uses pre-baked ground sprites in
  `assets/osrs-ground` rather than cache-native ground-layer blending.
- The post bloom is a cheap 8-tap screen-space glow, not a full HDR bloom chain.
- Static meshes are split into 16-tile chunks and frustum-culled, but there is
  no distance LOD or streaming yet; the map remains a resident 5×5 block.
- The shadow map covers dynamic units/structures/camps and large static props;
  small ground decorations still use cheap blob shadows. Chunked caster/LOD
  caching is pending.
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
- Cache texture alpha cut-outs are rendered opaque; leaf and flower sprites are
  wrapped and colour-lifted rather than alpha-tested.
- Fog/brush is client-side only; server-authoritative fog is required.
- No lobby, netcode, audio, or map editor.

## Next tasks

1. **M5.2 117HD Water**: water mesh, fresnel, sky reflection, specular, caustics.
2. **M5.3b ground blending**: cache-native terrain layers, per-tile texture
   IDs, anisotropic filtering, and alpha-tested foliage cut-outs.
3. **M5.4b post + draw distance**: full SSAO, anisotropic texture filtering,
   chunked streaming/LOD, and model batching.
4. Static scenery shadow caching/chunked casters.
5. Item selling/combining/passives and five champions with distinct kits.
6. Server-authoritative fog, economy, and shop transactions.
7. Deterministic 30 Hz sim tick in `crates/sim` + state hash.
8. Engine performance follow-ups: see [ENGINE_ROADMAP.md](ENGINE_ROADMAP.md).

## Verification log

- `cargo test -p rift-sim`: 7 tests (straight path, wall detour, nearest
  walkable, no corner cutting, simplification, bounds, LOS).
- `cargo test -p rift-cache`: 9 tests (smart reads, chained smart, XTEA, HSL,
  group rejection, framemap/frame parse, sprite bounds, texture reference).
- `cargo test -p rift-client`: 24 tests (5 items/layout, 2 font, 2 minimap,
  3 hero-select layout/cycling, 2 portrait raster, 2 wave formation,
  2 lane offset/distance, 4 scene/lighting, 2 shadow matrix/defaults).
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
- Hero select: three cache-model portraits baked; mouse/keyboard lock-in
  starts a fresh match with the chosen champion; `--champion` skips the screen.
- Scene scatter: 1511 scenery placements plus 1200 ground decorations; tree
  lines scaled 1.35–2.05× and cache props 1.35–1.5× over the old sizes.
- Wave formation: initial waves spawn in lane-relative wedges with per-unit
  parallel lane offsets and 0.35 s stagger; the test verifies at least 220
  units between base minions and the offset-lane test verifies parallel tracks.
- Model textures: 208 cache texture sprites uploaded to a 256-layer array;
  textured faces sample native leaf/wood/stone/flower art instead of flat
  fallbacks, with wood tree bases below the leaf canopy.
- Water: 9,712-triangle river surface with two animated ripple layers,
  fresnel sky reflection, sun specular and fog. Water is walkable and no longer
  an invisible collision wall.
- MSAA: the HDR scene target is 4x multisampled and resolves into the post/bloom
  chain; HUD remains single-sample and crisp.
- Static shadows: trees, ruins and pocket buildings cast into the 1024px sun
  shadow map alongside units.
- Combat presentation: jungle camp health slashed, critter camps are 150 HP /
  9 damage, camp bars are red and wider, and unit bars are pulled toward the
  camera to stop depth flicker while moving.
- Lane road: dirt-dominant, noise-warped 310-unit UVs remove the repeating
  cobble border while keeping the cache path sprite as a low-weight accent.
- Chunk culling: 400 terrain + 212 model + 49 water chunks. Release 1280×720
  immediate benchmark: p50 13.24 ms / p95 14.89 ms with shadows + 4× MSAA,
  p50 12.93 ms / p95 14.20 ms with shadows off, p50 12.17 ms / p95 13.88 ms
  with 1× MSAA. The `--benchmark <seconds>` harness prints these percentiles.
- Fog run: visible enemies 7–13; dark fog mesh over unseen tiles.
- Three-lane 20× run: victory states, towers/inhibitors/nexus live.
- Jungle demo: camp killed, buff Might 58s.
