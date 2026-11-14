# Engine road map

Last updated: 2026-11-14, after M5.23 (benchmark harness, chunk culling,
shadow proxies, terrain fetch reduction and cache-soil lanes).

This is the working performance/architecture backlog. It records what the
engine does today, what was measured, and the next changes that should give
the largest wins per unit of risk.

## 1. Current baseline

- Target: 1080p/60 on the AMD 680M iGPU; current captures are 1280×720.
- Map: 5×5 Karamja block, 614k terrain vertices, roughly 1.46M static model
  vertices, 800 ground decorations, 1,511 scenery placements.
- Assets: 208 cache-native model textures in a 256-layer array, 6 ground
  texture layers, 4× MSAA HDR scene target, 1024px sun shadow map.
- Simulation: click-to-move, A* pathing, hybrid dynamic-unit meshes, three
  lanes, minions, camps, towers and inhibitors.
- Renderer: one offscreen HDR target, bloom bright/blur/final passes, FXAA +
  grade + dither post, HUD after post.

### Measured (release, `--present-mode immediate`, 1280×720)

| Configuration (release, immediate) | Avg | p50 | p95 | approx FPS |
|---|---:|---:|---:|---:|
| Shadows + 4× MSAA | 11.19 ms | 13.24 ms | 14.89 ms | 89 |
| Shadows off + 4× MSAA | 10.94 ms | 12.93 ms | 14.20 ms | 91 |
| Shadows + 1× MSAA | 10.60 ms | 12.17 ms | 13.88 ms | 94 |
| Dev FIFO before chunk culling | — | — | — | 40 |
| Dev FIFO after chunk culling | — | — | — | 60 (vsync) |

The averages include fast frames where the app is idle; p50/p95 are the useful
numbers. Background applications and compositor scheduling add noise, so run
the benchmark with a clean desktop before using these as hard budgets.

Interpretation: static vertex load was the biggest single bottleneck and chunk
culling recovered most of it. The shadow map is now the next large cost
(roughly 5 ms at these settings), followed by 4× MSAA (roughly 1.5 ms) and the
terrain fragment shader. The current numbers are a dev-laptop measurement, not
a certified benchmark; use the harness described below before treating them as
budgets.

## 2. What M5.22 changed

### Static chunk culling

`ChunkedMesh` splits terrain, static models, water, blob shadows and static
shadow casters into world-space 16-tile AABB chunks at upload time. Triangles
that span multiple chunks (the distant backdrop) go into an always-visible
overlay bucket. Each frame the renderer extracts six frustum planes from the
camera view-projection and submits only intersecting chunks.

Current chunks/triangles:

| Mesh | Chunks | Triangles |
|---|---:|---:|
| Terrain | 400 | 204,800 |
| Static models | 212 | 483,606 |
| Water | 49 | 9,712 |
| Blob shadows | 173 | 3,662 |
| Static shadow casters | 170 | 413,513 |

### Shadow proxies and benchmark harness

The full static caster mesh was 413,513 triangles. `append_model_caster` now
emits a two-box trunk/canopy proxy for trees and a single AABB box for
buildings and ruins (38,028 triangles total). The camera-relative shadow pass
now costs roughly 0.3 ms p50 at the benchmarked view.

`--benchmark <seconds>` records frame times after a one-second warm-up and
prints avg/p50/p95/p99 plus approximate FPS. Combined with
`--present-mode immediate` and `--msaa 1|4`, this is the first P0 harness.

### Other recent engine work

- 4× MSAA on the HDR target with a single-sample resolve into post/bloom.
- Cache sprite (index 8) and texture-definition (index 9) decoders.
- Cache-native model textures with per-face planar UVs, alpha cutouts for
  leaves, and a bark-only path for dead/snowy trees.
- HD river surface with animated ripples, fresnel, sun specular and fog.
- Static scenery casters in the sun shadow map.
- `--present-mode fifo|immediate|mailbox` for uncapped profiling.
- `--msaa 1|4` for quality/performance comparisons.
- Water is walkable; structure footprints are automatically moved off lane
  centrelines; lane rendering is suppressed under water.

## 3. Current hot spots

1. **Shadow pass**: full static casters plus dynamic casters are submitted into
   a camera-relative 1024 map every other frame. Disabling shadows recovers
   roughly 5 ms in the release build.
2. **Terrain fragment cost**: base classification now samples four ground
   textures after dropping the redundant water fetch; dirt patch, road soil,
   slope and noise remain. 4× MSAA multiplies the cost.
3. **No GPU timing**: frame rate was measured through the window title. We need
   GPU timestamps and a repeatable benchmark scene before optimizing blind.
4. **CPU-side animation allocations**: `Model::animated_positions` allocates a
   fresh vector for each visible unit every frame. At current unit counts this
   is small, but it will matter for 10 heroes plus large waves.
5. **HUD geometry**: the HUD is rebuilt and uploaded every frame. Static
   panels and glyph runs could be cached; dynamic numbers still update.
6. **Repeated model draws**: the static mesh is merged, not instanced. Future
   scenery at larger draw distances will benefit from instancing and batching.
7. **No streaming/LOD**: the whole 5×5 block is resident. Chunk culling hides
   off-screen chunks, but there is no distance LOD or async load/unload.
8. **Startup cost**: decoding models, building chunk meshes and baking portraits
   is synchronous. Move to background jobs when the lobby and map streaming
   arrive.

## 4. Priority road map

### P0 — Instrumentation and performance gates (started)

- ✅ `--benchmark <seconds>` with avg/p50/p95/p99 frame logging.
- ✅ `--present-mode fifo|immediate|mailbox` and `--msaa 1|4` controls.
- [ ] Add CPU phase timing: update, mesh build, queue submission, shadow pass,
  scene pass, post, HUD.
- [ ] Enable `wgpu` timestamp queries where supported.
- [ ] Add golden performance smoke tests for chunk counts and draw counts.
- [ ] Make a 1080p capture the default benchmark before changing renderer code.

Acceptance: one command produces reproducible CPU/GPU frame budgets on the dev
machine, and regressions are visible in CI or a checked-in report.

### P1 — Shadow map cost (largely done)

- ✅ Tree trunk/canopy and building AABB shadow proxies cut the static caster
  mesh from 413,513 to 38,028 triangles.
- ✅ Benchmarked shadow-on vs shadow-off is now within noise at the test view.
- [ ] Cache static shadows by camera tile to eliminate even the proxy pass when
  the camera is stationary.
- [ ] Consider cascaded shadow maps for a larger camera-relative range.
- [ ] Keep dynamic casters at every-other-frame updates; interpolate where lag
  is visible at 1080p.

Acceptance: shadows-on remains within 1–1.5 ms of shadows-off at 1080p.

### P2 — Terrain fragment shader (started)

- ✅ Removed the redundant water-layer fetch underneath the water mesh.
- ✅ Stopped green ground from falling into the cobblestone/stone classifier.
- [ ] Pre-bake classification weights/top-two layers per terrain vertex instead
  of sampling four ground layers in the fragment shader.
- [ ] Move macro dirt patches and road masks into a low-resolution control
  texture.
- [ ] Profile 1080p with `--msaa 1 --shadows off`.
- Keep nearest filtering and the OSRS look; use mips, not expensive PBR.

Acceptance: terrain pass drops below 5 ms at 1080p and stays visually
indistinguishable in A/B captures.

### P3 — Geometry, LOD and streaming

- Add distance LODs for trees, ruins and ground decor (low-poly proxy meshes).
- Stream chunks around the camera instead of keeping the 5×5 block resident.
- Add terrain skirts at block/chunk boundaries to hide seams.
- Instance repeated scenery models and batch static draws.
- Add a GPU-driven or indirect path only after CPU draw submission shows up in
  the frame budget.

Acceptance: draw distance 2× the current horizon with stable 60 fps and no
visible chunk popping at the default camera.

### P4 — Animation and simulation

- Reuse per-unit vertex position buffers (`animated_positions` should write
  into a caller-owned Vec).
- Move unit updates to a fixed 30 Hz deterministic sim tick; interpolate render
  transforms between ticks.
- Convert the unit array to stable slots/IDs with deferred removal; avoid
  per-frame cloning and temporary Vecs in targeting/pathing.
- Cache pathfinding per lane/segment; A* only on target changes.
- Add a headless soak benchmark with thousands of ticks and a state hash.

Acceptance: 10 heroes + 100 minions + projectiles update under 2 ms/frame CPU
and produce identical hashes across runs.

### P5 — Post and quality settings

- Implement SSAO on a depth/normal prepass; keep a cheap fallback.
- Add a quality preset that selects MSAA 1/4, shadow resolution and bloom.
- Improve bloom to a proper mip chain and add an HDR exposure control.
- Add alpha-tested foliage shadow casters if the visual win is worth the cost.

Acceptance: SSAO and MSAA settings produce predictable frame budgets and can be
toggled without restarting a match.

### P6 — Engine/content pipeline

- Cache-native terrain layer blending (per-tile texture IDs, not the current
  six fixed layers).
- Async startup: cache decode, texture upload and chunk splitting on worker
  threads.
- Hot-reload champions/items/maps; validate references with `rift-tools`.
- Expand profiling to memory: static meshes, texture array, chunk overhead.

## 5. Profiling recipes

```sh
# Release, uncapped, current defaults
cargo run --release -p rift-client -- --present-mode immediate

# Fixed-duration frame-time stats, then exit
cargo run --release -p rift-client -- --present-mode immediate --benchmark 8

# Shadow cost
cargo run --release -p rift-client -- --present-mode immediate --shadows off

# MSAA cost
cargo run --release -p rift-client -- --present-mode immediate --msaa 1

# Bounded chunk draw counts
RUST_LOG=rift_client::renderer=info cargo run --release -p rift-client

# Simulation stress
cargo run --release -p rift-client -- --timescale 20 --shadows off --present-mode immediate
```

## 6. Design notes

- Keep scenery OSRS-flat-shaded and nearest-filtered. Do not introduce PBR to
  win a frame-time graph.
- Prefer chunk-level culling and LOD over shrinking the visible world.
- Every renderer optimization should have a visual A/B capture and a measured
  budget before it is considered done.
- The 1-tile = 128-unit contract and cache-native assets remain hard rules.
