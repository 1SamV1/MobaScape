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

## Current `map.json` (v1)

The first playable lane is `data/maps/karamja_rift/map.json`:

```json
{
  "name": "Karamja Rift - Mid Lane",
  "lane": [
    [2800, 3040], [2815, 3045], [2835, 3055], [2850, 3065],
    [2860, 3065], [2870, 3055], [2885, 3045], [2900, 3040]
  ],
  "wave_interval": 25.0,
  "structures": [
    { "team": 0, "kind": "nexus", "tile": [2800, 3040], "model": 1747,
      "health": 1200, "range": 0, "damage": 0, "interval": 0, "scale": 1.4 },
    { "team": 0, "kind": "tower", "tile": [2820, 3050], "model": 1761,
      "health": 450, "range": 640, "damage": 18, "interval": 1.2, "scale": 1.5 },
    { "team": 1, "kind": "tower", "tile": [2880, 3050], "model": 1761,
      "health": 450, "range": 640, "damage": 18, "interval": 1.2, "scale": 1.5 },
    { "team": 1, "kind": "nexus", "tile": [2900, 3040], "model": 1747,
      "health": 1200, "range": 0, "damage": 0, "interval": 0, "scale": 1.4 }
  ]
}
```

The scene clears collision in a ~4.5-tile corridor around the lane, blocks
3×3 footprints under structures, skips scenery within 5 tiles of the lane, and
draws a stone-path ribbon. The runtime spawns three melee skeletons and two
skeleton mages per team every `wave_interval` seconds.

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
