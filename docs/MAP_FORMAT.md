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

## Current `map.json` (v1.5)

The playable map is `data/maps/karamja_rift/map.json`:

- `nexus`: one structure per team (model 1747, 1500 HP).
- `lanes`: top, mid, bottom. Each lane has a `waypoints` polyline, four
  `towers` (two per team), and two `inhibitors` (one per team).
- `wave_interval`: 25 s. Each lane spawns two melee skeletons and one skeleton
  mage per team per wave.
- `camps`: four neutral jungle camps. Each camp has a tile, model, optional
  idle/walk sequence, health, respawn time, buff (`might` or `swift`), and
  scale. Camps grant a 60 s team buff when killed.
- Towers: model 1761, 450 HP, range 640, 18 damage / 1.2 s.
- Inhibitors: model 1747, 700 HP, no attack. At least one enemy inhibitor must
  fall before the nexus can be damaged.
- Nexus: model 1747, 1500 HP. Destroying it ends the match.

The scene clears collision in a ~4.5-tile corridor around every lane, blocks
3×3 footprints under structures, skips scenery within 5 tiles of any lane, and
draws a stone-path ribbon per lane. Positions are absolute world tiles (e.g.
2800, 3040), not region-local coordinates.

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
