# Architecture decisions

Short ADRs. Add a new entry when a decision is hard to reverse or affects
multiple crates.

## ADR-001 — Pivot from the 2006Scape Java client/server to Rust

**Status:** accepted.

The 2006Scape Java client/server was built for OSRS-style tick gameplay and a
317 cache. It is too limited for fast 5v5 MOBA pacing, and reskinning it fights
the renderer and interface code. We keep the Java work as reference only
(`docs/reference/`) and build a custom Rust engine.

Consequences: new workspace, new cache pipeline, no Java build in CI.

## ADR-002 — Use a pinned modern OSRS cache (OpenRS2 2392 / build 235)

**Status:** accepted.

Build 235 has a complete XTEA key set on OpenRS2; builds 241+ currently report
zero archived keys, which blocks map-location extraction. Modern content and
models are still available. The cache is gitignored; users fetch it locally.

Consequences: map extraction is reproducible; new content requires a deliberate
version bump and key audit.

## ADR-003 — Custom wgpu stack, not a full engine

**Status:** accepted.

We use `wgpu` + `winit` + `glam` + `hecs` (planned) + `egui` (planned) instead
of Bevy. The OSRS look depends on flat per-face shading, vertex animation, and a
bespoke terrain path; owning the renderer is simpler than fighting a PBR engine.

Consequences: we build UI/assets/audio plumbing ourselves; the renderer stays
small and controllable.

## ADR-004 — NPC/boss models as the first champion roster

**Status:** accepted for v1.

Player-kit compositing (`idk` body parts + equipment models + recolours) is a
large subsystem. Distinct boss models (Jad, Zulrah, Vorkath, etc.) give visual
identity and recognizable silhouettes immediately. Player avatars/gear become a
later cosmetic phase.

## ADR-005 — Left-click tap-to-move + hold-to-steer

**Status:** accepted (user decision).

- Tap LMB on ground: pathfind to the point.
- Hold LMB: steer continuously toward the cursor direction from the champion.
- Click an enemy: attack it.
- QWER: abilities; Shift self-cast; RMB attack-move/force-attack.

Consequences: the input layer needs both path following and direct steering;
the server validates both. Camera edge-pan/minimap remains.

## ADR-006 — Karamja carve + internal map editor

**Status:** accepted (user decision).

Start from a 3×3 Karamja region block and carve a MOBA arena, rather than
hand-authoring terrain from scratch. Build an internal editor with procedural
height/river/scatter and a cached asset library so more maps are cheap.

Consequences: terrain import, height blending, collision rebake, and an editor
UI are M1/M5 work. Map data is split into design JSON + derived bakes.

## ADR-007 — No game assets in the repository

**Status:** accepted.

The repo contains code, design data, and docs only. Cache files, extracted
models/textures/maps/sounds, and screenshots of game assets are gitignored. Users
run the extractor locally. Keep the repo private until distribution policy is
decided.

## ADR-008 — Flat shading first, textures later

**Status:** accepted.

Most OSRS scenery reads through vertex colours and per-face light. v1 renders
flat-shaded models with a green fallback for textured faces; texture sampling,
alpha-tested foliage, and animated textures come after the core match loop.

Consequences: the first visual pass is fast and recognizable; textured content
will look approximate until index 9 is decoded.

## ADR-009 — Server-authoritative 30 Hz with client prediction

**Status:** accepted.

The server owns the sim. The client predicts own movement and reconciles; remote
units are interpolated. Fog is filtered server-side. Direct-connect room codes
precede any matchmaking service.

Consequences: `crates/sim` must be deterministic and shared; netcode tests need
simulated latency/loss from day one.
