# AGENTS.md — working on MobaScape

This file is the contract for AI agents and new contributors. Read it before
touching code. When in doubt, prefer a smaller change that keeps the workspace
building and the cache reader testable.

## 1. What this project is

MobaScape is a 5v5 MOBA with an Old School RuneScape visual identity, built on a
custom Rust engine. It is **not** a RuneScape private server and **not** a
reskin of the 2006Scape Java client. The Java experiment is archived under
`docs/reference/` and is never built.

North star:

- 5v5 real-player PvP; bots are dev-only.
- Server-authoritative simulation at 30 Hz.
- Click-to-move (tap) plus hold-to-steer, QWER abilities.
- Cache-native terrain, scenery, models, animations, icons, and fonts.
- Data-driven champions/items/maps, hot-reloadable.
- Total conversion: boot directly into the MOBA.

## 2. Hard rules

1. **Never commit Jagex assets or caches.** `.cache/`, `assets/`, extracted
   models, textures, maps, sounds, and screenshots of game assets are
   gitignored. Do not add them. The repo ships code and docs only.
2. **Do not trust the client.** Damage, vision, cooldowns, gold, pathing,
   collision, and match results are server-owned. The client predicts only its
   own movement and cosmetics.
3. **The sim crate stays deterministic.** No wall-clock time, no OS RNG, no
   floating-point platform assumptions in `crates/sim`. Use a seeded RNG and
   fixed timestep. Headless matches must produce identical state hashes.
4. **Bots are not production gameplay.** Bots may fill dev matches and soak
   tests; a production match requires 10 humans.
5. **OSRS identity is a constraint, not a suggestion.** 1 tile = 128 world
   units; cache heights are ×8; flat per-face shading; nearest-neighbor textures
   when textures are added. Do not turn the game into a PBR fantasy renderer.
6. **Do not revive the Java server/client.** Port ideas, not code. The Java tree
   is reference only.
7. **Ask before creative direction changes.** Champion roster, map layout, HUD
   look, itemization, and game-feel changes are user decisions. Technical
   refactors and bug fixes do not need a check-in.

## 3. Repo map

```
crates/cache/    cache reader + decoders (terrain, locations, configs, models)
crates/tools/    rift-tools CLI (inspect, dump, export OBJ)
crates/client/   winit + wgpu client (world, animation, HUD, lane, combat)
crates/sim/      deterministic tile grid, collision, A* pathfinding
data/maps/       authored lane waypoints, structures, wave timing
crates/data/     (planned) serde content types + hot reload
crates/proto/    (planned) wire messages + snapshot encoding
crates/server/   (planned) match instances, lobby, auth-lite
crates/render/   (planned) shared OSRS-style renderer
crates/ui/       (planned) HUD, shop, scoreboard, lobby, minimap
crates/editor/   (planned) map editor + procedural generator
crates/bots/     (planned) dev slot fillers and headless soak tests
crates/mcp/      (planned) dev MCP server
data/            (planned) champions, abilities, items, maps, waves, UI
docs/            architecture, asset pipeline, roadmap, ADRs
docs/reference/  archived Java-era work; never built
```

## 4. Build and test

```sh
cargo check --workspace
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings   # target: clean
```

M1 commands:

```sh
cargo run -p rift-tools -- cache-info
cargo run -p rift-tools -- terrain 44 47
cargo run -p rift-tools -- locations 44 47
cargo run -p rift-tools -- object 1282
cargo run -p rift-tools -- find-npc monkey --limit 5
cargo run -p rift-tools -- npc 23
cargo run -p rift-tools -- sequence 222
cargo run -p rift-tools -- animate 7744 222
cargo run -p rift-tools -- model 1715
cargo test -p rift-sim
cargo run -p rift-client
cargo run -p rift-client -- --demo-combat
cargo run -p rift-client -- --demo-camp --timescale 4
cargo run -p rift-client -- --demo-kit --timescale 4
cargo run -p rift-client -- --timescale 20
```

Cache location: `.cache/osrs-235/cache` by default. Pass `--cache` and
`--keys` to the tools if yours lives elsewhere. See
`docs/ASSET_PIPELINE.md`.

## 5. Coding conventions

- Rust 2024 edition, `rustfmt`, 4 spaces, no tabs.
- Libraries use `thiserror`; binaries use `anyhow`.
- Use `tracing` (`info!`, `warn!`, `debug!`), never `println!` in library code.
  `rift-tools` may print human-readable output.
- Prefer small modules with explicit data types over trait-heavy abstractions.
- Keep the cache reader allocation-conscious: it is the hot path for asset
  builds.
- Do not add a dependency without a reason. No `unsafe` outside the mmap in
  `crates/cache/src/store.rs` (and document it).
- Public functions get doc comments when the semantics are not obvious.
- Add a regression test for every decoder bug. Cache fixtures should be tiny
  byte arrays or generated groups, not committed game assets.

## 6. How to add things

### Add a cache decoder

1. Port the reference loader from RuneLite/OpenRS2 (BSD-2-Clause) with a comment
   and keep the field names close to the reference.
2. Put it in `crates/cache/src/<name>.rs`, export it from `lib.rs`.
3. Add a `rift-tools` subcommand to inspect it.
4. Verify against one known asset visually (see asset pipeline checklist).
5. Add unit tests for smart reads, opcode skipping, and bounds.

### Add a model/rendering feature

1. Decode into a plain data struct in `crates/cache`; do not put wgpu types in
   the cache crate.
2. Build engine-space vertices in `crates/client/src/scene.rs` (or later
   `crates/render`). Y is up in engine space; cache model Y is flipped.
3. Flat shading: one face = one shade, derived from the face normal. No
   smoothing groups until we deliberately add them.
4. Keep the `Vertex` layout minimal: position + color. Add attributes only with
   a measured need.

### Add a champion or item

1. Add JSON under `data/champions/` or `data/items/` (schema TBD in M1).
2. Reference cache IDs, not baked meshes: `npc`, `model`, `spotanim`, `obj`.
3. Keep ability logic in `crates/sim`; JSON only carries numbers and refs.
4. Run `rift-tools` validation and a headless bot match before opening a PR.

### Add a map

1. Use the editor (M5) or the map JSON schema in `docs/MAP_FORMAT.md`.
2. Never hardcode spawns, lanes, tower positions, or camps in Rust.
3. Bake collision/fog/nav separately from the display mesh.
4. Validate connectivity and spawn clearance before committing.

## 7. Testing requirements

- `cargo test --workspace` must pass.
- Cache changes: add a focused unit test; do not require the full cache in CI.
- Sim changes: add or update a golden state-hash test.
- Netcode changes: test with simulated 0/50/150 ms RTT and 5% loss.
- Renderer changes: include a headless screenshot or a manual visual check note.
- Performance: 1080p/60 target on the AMD 680M iGPU. Profile before optimizing.

## 8. Known M3.9 limitations

- Client renders a 3×3 region block (192×192 tiles); there is no terrain skirt
  or streaming beyond it, and the collision grid is approximate (water + object
  footprints, no cliffs/bridges).
- Three lanes with two towers and one inhibitor per lane, plus one shielded
  nexus per team. Four jungle camps grant Might (+25% damage) or Swift (+20%
  speed) for 60 s. No brush, fog, or items.
- Nexus is damageable only after at least one enemy inhibitor falls.
- The HUD is geometry only (bars, four ability boxes, crosshair, nexus/wave
  bars, minimap, buff bar); all numbers live in the window title. No text
  renderer yet.
- Champion kit: Q fire blast, W blink, E heal+shield, R meteor, with mana and
  cooldowns. No passive or item shop; gold/XP are tracked but gold is unspent.
- Model formats 1 and old-format models are not implemented yet.
- Textures are not sampled; textured faces and the `0xff00ff` overlay sentinel
  use flat fallback colours. Dead-tree trunks read as pale spikes.
- Animaya is not applied; only classic frame/framemap animation works.
- No lobby, netcode, audio, or map editor. Ambient NPCs idle only.
- LZMA groups are implemented but not exercised by the pinned cache.
- Config archives use real file IDs from the reference table; keep it that way.
- The viewer follows the player by default. `WASD` disables follow; `F` toggles.
- Use ASCII in window/UI strings until fonts are wired.

## 9. Commit / PR style

- Commits: `area: imperative summary` (e.g. `cache: decode type 2 model offsets`).
- One logical change per commit; include the test that proves it.
- PR description: what changed, how it was verified, known gaps, and any asset
  IDs used for manual checks.
- Do not mix refactors with behavior changes.
- **End every task with a commit and push.** Before reporting completion, run
  `git status`. If there are changes, commit them with a scoped message and run
  `git push origin main`. Never leave a task with uncommitted work unless the
  user explicitly asks you to. Verify with `git status` (clean) and
  `git ls-remote --heads origin main` (matches local `HEAD`).

## 10. Escalation

Ask the user before:

- changing the champion roster, map layout, camera, or control scheme;
- adding a new external service, account system, or telemetry;
- committing anything derived from Jagex assets;
- making the repository public or changing its license;
- large dependency or architecture changes.
