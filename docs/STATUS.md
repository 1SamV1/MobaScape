# Status

Last updated: M0 prototype, 2026-11-12.

## Working

- Rust workspace builds (`cargo check --workspace`).
- `rift-tools cache-info`: reads all 25 indices, reference tables, and keys.
- `rift-tools terrain 44 47`: decodes Karamja terrain, prints height map and stats.
- `rift-tools locations 44 47`: decodes 3,102 placements and resolves object names.
- `rift-tools object/underlay/overlay/find-object`: config inspection.
- `rift-tools model 1570/1617/1715`: model types 3 and 2.
- `rift-tools export-obj`: OBJ export for Blender checks.
- `rift-client`: winit + wgpu viewer; renders one 64×64 Karamja region with
  terrain, underlay/overlay colours, and 320 scenery placements (palms, trees,
  plants). Camera pan/rotate/zoom works.

## Verified numbers

- Cache: OpenRS2 2392 / build 235; 167 MB; 2,668 XTEA keys.
- Terrain region 44,47: 46,379-byte group, 16,384 tiles, 7 overlays, 4,096
  underlays.
- Locations region 44,47: 6,506 bytes, 3,102 placements.
- Object 1282 "Dead tree" → model 1715 (271 verts, 496 faces, untextured).
- Object 1276 "Tree" → model 1570 (90 verts, 110 faces, 12 texture faces).
- Scene: 4,225 terrain vertices, 174,606 model vertices, 320 placements.

## Known issues

- The viewer renders a single region as a floating island; no neighbouring
  regions or skirt yet.
- Terrain colour is an approximation: underlay/overlay RGB × a height shade.
  OSRS's palette/light table is not implemented, so greens are brighter than
  the game.
- Textured model faces use a green fallback; index 9 textures are not sampled.
- No animations; sequences/frames and Animaya are not decoded yet.
- No player kits/equipment compositing. Scenery only.
- Model type 1 and old-format models return an error.
- LZMA decompression is implemented but untested against this cache.
- Config archive file IDs are assumed contiguous (`enumerate` → ID). Verify
  before relying on non-contiguous config archives.
- Window title contains a Unicode em dash that can render mojibake under X11;
  use ASCII in UI strings until fonts are wired.
- The viewer is not a game: no sim, netcode, HUD, lobby, audio, or editor.

## How to reproduce the screenshot check

```sh
cargo build -p rift-client
WINIT_UNIX_BACKEND=x11 WAYLAND_DISPLAY= DISPLAY=:0 ./target/debug/rift-client &
# raise the window, then:
spectacle -b -n -o /tmp/rift.png
```

Do not commit screenshots of Jagex assets.

## Next tasks

1. Sequence/frame decoding and CPU vertex animation for one NPC.
2. Sprite/font extraction for HUD and item icons.
3. 3×3 Karamja import + terrain skirt + first lane carve.
4. `crates/sim` skeleton: fixed tick, movement, A*, state hash.
5. Map data schema + editor minimum carve/place/export.
6. Fix config file-ID mapping and colour/lighting fidelity.

## Verification log

- `cargo check --workspace` passes.
- `rift-tools model 1617` fixed a type-2 offset bug (face opcodes now precede
  priorities/packed groups; deltas precede colours).
- `rift-client` scene build completes and renders; wgpu 30 API changes handled
  (`CurrentSurfaceTexture`, `Queue::present`, Option depth state, `multiview_mask`).
