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

## `map.json` sketch

```json
{
  "name": "Karamja Rift",
  "version": 1,
  "size": [160, 160],
  "source": {
    "cache": "openrs2-2392-build235",
    "regions": [[43, 46], [44, 46], [45, 46], [43, 47], [44, 47], [45, 47], [43, 48], [44, 48], [45, 48]],
    "carve": "karamja_rift_v1"
  },
  "teams": [
    { "id": 0, "name": "West", "color": [80, 140, 220], "fountain": [18, 80], "nexus": [26, 80] },
    { "id": 1, "name": "East", "color": [220, 90, 70], "fountain": [142, 80], "nexus": [134, 80] }
  ],
  "lanes": [
    {
      "id": "top",
      "width": 10,
      "waypoints": [[26, 80], [30, 120], [80, 140], [130, 120], [134, 80]]
    },
    {
      "id": "mid",
      "width": 10,
      "waypoints": [[26, 80], [80, 80], [134, 80]]
    },
    {
      "id": "bot",
      "width": 10,
      "waypoints": [[26, 80], [30, 40], [80, 20], [130, 40], [134, 80]]
    }
  ],
  "towers": [
    { "team": 0, "lane": "top", "tier": 1, "pos": [34, 112], "npc": 0 },
    { "team": 0, "lane": "top", "tier": 2, "pos": [28, 96], "npc": 0 }
  ],
  "inhibitors": [
    { "team": 0, "lane": "top", "pos": [26, 84] }
  ],
  "camps": [
    { "id": "blue", "pos": [48, 96], "npc": 0, "buff": "prayer", "respawn": 120 },
    { "id": "red", "pos": [112, 64], "npc": 0, "buff": "magic", "respawn": 120 },
    { "id": "boss", "pos": [80, 80], "npc": 0, "buff": "boss", "respawn": 480 }
  ],
  "brush": [[[40, 90], [52, 90], [52, 102], [40, 102]]],
  "fog_blockers": [[[44, 94], [50, 94], [50, 100], [44, 100]]],
  "spawns": {
    "team0": [[18, 80], [19, 80], [20, 80], [21, 80], [22, 80]],
    "team1": [[142, 80], [141, 80], [140, 80], [139, 80], [138, 80]]
  }
}
```

All positions are tile coordinates on the 160×160 grid, origin at the
south-west corner. `width` is in tiles. Tower/camp NPC IDs reference
`data/npcs/` or a future `data/units/` table; they are not raw cache IDs.

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
