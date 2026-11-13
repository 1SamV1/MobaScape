# Map format (planned)

The map is data, not Rust. A map has a display mesh, a collision/nav bake, a fog
grid, and gameplay markers. The editor writes the same files the runtime reads.

## Directory layout

```
data/maps/karamja_rift/
  map.json          # metadata, markers, lanes, camps, towers, fog zones
  terrain.bin       # baked tile heights/colours/flags (versioned)
  collision.bin     # per-tile walkable/blocked, per plane
  nav.bin           # A* graph / clearance data
  minimap.png       # baked minimap (local only; not committed if Jagex-derived)
  scenery.bin       # optional static placement list, if not in map.json
```

`terrain.bin`, `collision.bin`, `nav.bin`, and `minimap.png` are derived from
the cache and are **not committed**. `map.json` is committed because it is our
design data; it references cache IDs, not assets.

## Current `map.json` (v1.6)

The playable map is `data/maps/karamja_rift/map.json`:

- `nexus`: one structure per team (model 1747, 1500 HP).
- `lanes`: top, mid, bottom. Each lane has a `waypoints` polyline, four
  `towers` (two per team), and two `inhibitors` (one per team).
- `wave_interval`: 25 s. Each lane spawns two melee skeletons and one skeleton
  mage per team per wave.
- `camps`: eight neutral jungle camps. Each camp has a tile, model, optional
  idle/walk sequence, health, respawn time, buff (`might` or `swift`), scale,
  and its AI numbers: `damage`, `range`, `interval`, `speed`, `aggro` and
  `leash`. Camps aggro when a hero enters `aggro` or when they take damage,
  chase the hero, leash back to `home` when pulled past `leash`, heal to full
  on arrival and respawn after `respawn` seconds. Camps grant a 60 s team buff
  when killed.
- `biomes`: regional biome pockets. Each pocket transplants a rectangle of a
  real OSRS region into the map:
  - `region`: `[region_x, region_y]` of the source OSRS region.
  - `rect`: `[x, y, width, height]` tile rectangle inside that region.
  - `origin`: map tile where the rectangle is placed.
  - `tint`: ground colour multiplier for the pocket.
  - `side`: `radiant` or `dire`, `label`: the name shown in the HUD.
  The dire half holds dark magic (Wilderness dead-tree graveyard), vampires
  (Morytania swamp), ogre ruins (ceremonial pillars) and a goblin camp; the
  radiant half holds a human city (Varrock), a human keep, the elven lands
  (Tirannwn) and the gnome stronghold. Pocket terrain, colours and scenery all
  come from the cache.
- `watchtowers`: per-lane structures with `kind: "watchtower"`, team, tile,
  model, health, `range` (the long lock range), `damage`, `interval`, `scale`
  and `aggro` (the short acquire radius). A watchtower acquires the first
  enemy inside `aggro`, then keeps firing slow bolts at that target until it
  dies or leaves `range`. Its bolts hit heroes, lane minions and jungle camps.
- `river`: the waterway that splits the rift. `points` is a polyline in map
  tiles running from the upper left third through the centre to the lower
  right third, `width` is the half width in tiles, `depth` how far the bed is
  cut, `bridges` lists the tiles that stay walkable (the centre crossing) and
  `levels` sets the water height at the first and last point.
- `brush`: rectangles `[x0, y0, x1, y1]` in world tiles. Units inside brush are
  hidden from the enemy unless the player is within 500 units.
- `atmosphere`: optional lighting/atmosphere block:
  `sun_dir`, `sun_color`, `ambient_sky`, `ambient_ground`, `fog_color`,
  `fog_density`, `fog_height`, `sky_top`, `sky_horizon`, `ground_fog`.
  Values are sRGB colours and world-space units; the renderer converts them to
  linear light.
- Towers: model 1761, 450 HP, range 640, 18 damage / 1.2 s.
- Inhibitors: model 1747, 700 HP, no attack. At least one enemy inhibitor must
  fall before the nexus can be damaged.
- Nexus: model 1747, 1500 HP. Destroying it ends the match.

The scene clears collision in a ~4.5-tile corridor around every lane (after
biome pockets are placed, so a pocket never seals a lane), blocks footprints
under structures, skips scenery within 5 tiles of any lane, and draws a dirt
path ribbon per lane. Positions are absolute world tiles (e.g. 2800, 3040),
not region-local coordinates.

Lane sides are scattered procedurally with OSRS tree models: a deterministic
xorshift RNG (fixed seed `0x5EED9E3779B97F4A`) walks each lane with variable
spacing (170–470 units), random offsets (4.9–9.8 tiles), random scales and
occasional clusters and gaps. Species depend on the side of the river: dead
and dark trees on the dire side, lush green canopy on the radiant side.
Broken OSRS castle walls and ruin clusters (models 16918, 9233 and 9625) are
scattered the same way 10.5–15 tiles out. The seed is fixed, so every match
starts from the identical world. Pocket scenery is never placed within 6.5
tiles of a lane, so a door or a wall can never end up in the middle of the
road.

Lanes are **baked into the terrain**, not drawn as a ribbon. The scene
computes a smooth lane mask with a noise-wobbled edge, stores it in the
terrain vertex alpha and tints the ground toward the OSRS path colour.
`scene.wgsl` blends the OSRS dirt and path sprites into the terrain by that
mask, so the road is seamless with the world and can never z-fight with the
ground.

## Future schema

Top/bottom lanes, inhibitors, jungle camps, brush, fog blockers, and team
spawns will be added as the map editor lands. Positions are absolute world
tiles (e.g. 2800, 3040), not region-local coordinates.

## Terrain bake

`terrain.bin` is a header + per-plane arrays:

```
magic "RIFTTERR" | version u32 | width u32 | height u32 | planes u32
heights: u8[planes][h][w]         # cache height, world = *8
underlay: u16[planes][h][w]
overlay: u16[planes][h][w]
overlay_path: u8[planes][h][w]
overlay_rotation: u8[planes][h][w]
settings: u8[planes][h][w]
```

The display mesh is built from the same data; collision uses `settings` plus
object footprints. Bridge/plane data is kept separate from display meshes.

## Collision and nav

- Collision is per tile, per plane: blocked, bridge, water, cliff, brush.
- Object footprints come from `ObjectDefinition.size_x/size_y`, shape, and
  rotation; walls/doors use `wall_or_door` and `blocking_mask`.
- A* operates on tiles with clearance; lanes are corridors with a preferred
  width. The server owns pathing; the client predicts along the returned path.
- Validate: spawns not blocked, each lane connected, towers reachable, camps
  reachable, no path through cliffs/water, no spawn-to-spawn straight line.

## Fog

- `fog_blockers` are canopy/brush polygons; they block vision but not movement
  (unless collision says otherwise).
- `brush` polygons hide units inside them from the enemy team.
- The server computes per-team visibility at 10 Hz; the client renders fog from
  the same grid plus its local camera.

## Karamja carve plan

1. Import a 3×3 region block around 44,47 (192×192 tiles) and clamp to a
   160×160 playable area.
2. Draw top/mid/bot lane polylines. Flatten a ~10-tile corridor, blend edge
   heights, clear blocking locs, paint lane ground.
3. Carve a river crossing; add bridge objects; keep water collision.
4. Place bases (west/east), fountains, nexus, two towers per lane, inhibitors.
5. Paint canopy/brush fog blockers and jungle camp pockets.
6. Scatter procedural foliage/buildings outside lanes from the cache library;
   curate the base/river banks manually.
7. Bake collision/nav/fog/minimap and validate.

## Editor (planned)

- Same renderer as the client, free camera.
- Modes: import region, sculpt height, paint underlay/overlay, paint collision,
  paint fog, place/rotate/scale objects, lane drawing, marker placement,
  validation overlay.
- Asset library: searchable cached locs/npcs/models with thumbnails.
- Procedural generator: seeded FBM height, river, biome splat, scatter rules.
- Export writes `map.json` + bakes; hot reload lets the client see changes live.


## Fog of war

Fog is not authored in the map. The renderer receives the hero position and a
1500 world unit vision radius every frame and darkens the scene by horizontal
distance in `scene.wgsl`. The visibility grid (used for unit/projectile
culling and the minimap) is still built from team line of sight.
