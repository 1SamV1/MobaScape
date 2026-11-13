# Contributing

This is a private, early-stage project. Read [AGENTS.md](../AGENTS.md) first; it
is the source of truth for rules and conventions. This file covers the practical
setup.

## Setup

1. Install Rust (stable, 1.85+).
2. Get the pinned cache (OpenRS2 2392 / build 235) and put it at
   `.cache/osrs-235/cache/` with `keys.json` next to `main_file_cache.dat2`.
   See [ASSET_PIPELINE.md](ASSET_PIPELINE.md).
3. Verify:

```sh
cargo check --workspace
cargo test --workspace
cargo run -p rift-tools -- cache-info
```

## Useful commands

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p rift-tools -- terrain 44 47
cargo run -p rift-tools -- locations 44 47
cargo run -p rift-tools -- model 1715
cargo run -p rift-tools -- export-obj 1715 /tmp/tree.obj
cargo run -p rift-tools -- find-npc monkey --limit 5
cargo run -p rift-tools -- sequence 222
cargo run -p rift-tools -- animate 7744 222
cargo test -p rift-sim
cargo run -p rift-client
cargo run -p rift-client -- --demo-combat
cargo run -p rift-client -- --timescale 20
cargo run -p rift-client -- --demo-camp --timescale 4
cargo run -p rift-client -- --demo-kit --timescale 4
# Populate gitignored UI assets first: see docs/UI_ASSETS.md
cargo run -p rift-client -- --graphics 117hd
cargo run -p rift-client -- --graphics classic
cargo run -p rift-client -- --graphics 117hd --debug-view lighting
cargo run -p rift-client -- --graphics 117hd --debug-view shadow
cargo run -p rift-client -- --graphics 117hd --debug-view ao
cargo run -p rift-client -- --graphics 117hd --shadows map
cargo run -p rift-client -- --graphics 117hd --pitch 0.22
cargo run -p rift-client -- --demo-shop --timescale 4
cargo run -p rift-client -- --demo-base --timescale 4
cargo run -p rift-client -- --demo-inhibitor --timescale 20
cargo run -p rift-client -- --champion jad
cargo run -p rift-client -- --champion kree
cargo run -p rift-client -- --demo-target 2900 3060
```

For a deterministic viewer screenshot under XWayland:

```sh
WINIT_UNIX_BACKEND=x11 WAYLAND_DISPLAY= DISPLAY=:0 ./target/debug/rift-client &
# raise the "Rift" window, then:
spectacle -b -n -o /tmp/rift.png
```

Do not commit cache files, extracted assets, or screenshots of game assets.

## Code style

- Rust 2024, `rustfmt`, 4 spaces.
- `thiserror` in libraries, `anyhow` in binaries.
- `tracing` for logs; no `println!` in libraries.
- Small modules and explicit structs over trait-heavy abstractions.
- Document non-obvious cache offsets and magic constants.
- No new `unsafe` except the mmap in `crates/cache/src/store.rs`.

## Tests

- Decoder changes need a focused unit test with a tiny fixture.
- Sim changes need a golden state hash.
- Netcode changes need a loss/jitter test.
- Renderer changes need a manual visual check note (and ideally a headless
  screenshot in CI later).
- Do not make CI depend on the full OSRS cache.

## Pull requests

Include:

- What changed and why.
- How it was verified (commands, cache/model IDs, screenshot path).
- Known gaps and follow-ups.
- Any user-facing creative decision that needs approval.

Keep commits logical and scoped. Do not mix refactors with behavior changes.

## Task completion

Every task ends with a clean, pushed repository:

```sh
git status                 # must be clean
git log --oneline -1       # the task's commit
git push origin main       # if not already pushed
git ls-remote --heads origin main  # must match local HEAD
```

Do not leave uncommitted work at the end of a task unless the user explicitly
asks you to stop before committing.

## Adding a decoder

1. Port the reference loader from RuneLite/OpenRS2 (BSD-2-Clause), keeping field
   names close to the reference and adding a provenance comment.
2. Add bounds checks and clear errors with cache IDs.
3. Add a `rift-tools` command.
4. Verify one real asset visually.
5. Add a unit test for smart reads/opcode skipping.

## Adding content

- Champions/items/maps live in `data/` (schema lands in M1).
- Reference cache IDs, never bake meshes into code or JSON.
- Validate with `rift-tools` and a headless bot match before merging.
- Creative changes (roster, map layout, HUD) need the user's approval.

## Reporting bugs

Include:

- command/crate and commit hash;
- cache snapshot/build;
- cache index/archive/group/model ID;
- expected vs. actual;
- minimal reproduction (a tiny fixture if possible).

## License

By contributing, you agree your code is licensed under the repository's MIT
license. Do not contribute Jagex assets or code you cannot license.
