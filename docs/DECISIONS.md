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

## ADR-010 — A* on an integer tile grid in `crates/sim`

**Status:** accepted.

Pathfinding lives in the deterministic sim crate, not the renderer. The grid is
integer tiles; movement costs are 10/14; diagonals may not cut blocked corners.
The client uses the same code for click-to-move; the server will own the
authoritative path. Collision is currently conservative (water + object
footprints) and will be replaced by authored map collision.

## ADR-011 — Screen-space HUD via a second pipeline

**Status:** accepted.

The HUD uses the same vertex format as the world but a second pipeline with an
identity camera, alpha blending, and depth testing disabled. This keeps the HUD
cheap and avoids a UI framework dependency. Text is deferred; numeric values are
shown in the window title until a bitmap font atlas is added.

## ADR-012 — One data-driven mid lane first

**Status:** accepted.

The first playable MOBA loop is a single mid lane defined in
`data/maps/karamja_rift/map.json`. Scene building clears a collision corridor,
blocks structure footprints, skips scenery near the lane, and emits a stone-path
ribbon. `AnimatedScene` spawns waves, runs minion/tower AI, and ends the match
when a nexus dies. Top/bottom lanes, inhibitors, jungle camps, and brush are
deferred until the loop is stable.

## ADR-013 — Three lanes + inhibitor-gated nexus

**Status:** accepted.

The map now has top/mid/bottom lanes, two towers and one inhibitor per lane per
team, and one nexus per team. A nexus is damageable only after at least one
enemy inhibitor is destroyed. Minions spawn per lane (two melee, one ranged) and
follow that lane's waypoints. This creates a real push/defend loop without
requiring items or a full champion kit yet.

## ADR-014 — Jungle camps, team buffs, and a geometry minimap

**Status:** accepted.

Four neutral camps sit in the jungle pockets. They respawn and grant the
attacker's team a 60 s buff: Might (+25% damage) or Swift (+20% speed). Camps
are passive in v1 (no retaliation) and use single-model NPCs. The minimap is
drawn with HUD quads from live state (lanes, structures, camps, minions,
player); it is display-only until click-to-pan lands. Fog of war remains a
server-side milestone.

## ADR-015 — QWER champion kit and arrow-key camera

**Status:** accepted.

The player now has a four-ability kit: Q projectile, W blink, E heal/shield,
R meteor. Mana, cooldowns, XP, gold, levels, and shield absorption are tracked.
To free Q/W/E/R, camera panning moved to the arrow keys, rotation to Z/X, and
recenter/follow to Space. R is the ultimate, not camera reset. The HUD draws four
ability boxes with cooldown fills; the title carries the numeric values.

## ADR-016 — Client-side fog/brush prototype

**Status:** accepted for the prototype.

A 192×192 visibility grid is computed from allied units and structures. Enemy
and neutral units outside vision are hidden; brush rectangles hide units unless
the player is within 500 units; an opaque dark mesh covers unseen tiles. This is
client-side only and must move to the authoritative server before multiplayer.
The minimap hides unseen units but does not yet draw fog or support click-to-pan.

## ADR-017 — Data-driven champions, starting with three boss avatars

**Status:** accepted.

Champion data lives in `data/champions/*.json`: model/idle/walk IDs, scale,
health, mana, damage, range, speed, ranged-auto flag, and Q/R/E/W numbers. The
client loads one champion at launch with `--champion monkey|jad|kree`. The first
roster uses OSRS boss/mini-boss models (Monkey, JalRek-Jad, Kree'arra Jr.) and
shares the Q/W/E/R shape; a champion-select UI, passives, and more distinct kits
come later.

## ADR-018 — Six-slot fountain shop with flat-stat items

**Status:** accepted for the prototype.

Items live in `data/items/items.json` with an id, name, HUD colour, gold cost,
and flat damage/health/mana/speed/ability values. The player has six inventory
slots. Buying is allowed only within 900 world units of the spawn fountain, so
the shop is a base interaction rather than a global menu. `B` toggles the panel;
`1`–`6` and row clicks buy; clicking outside or `Esc` closes. Ability power is a
percentage that scales Q/R damage and E heal/shield. The client applies the
transaction locally in this prototype; the server must own gold, range,
inventory, and stats before multiplayer.

## ADR-019 — Built-in 5x7 bitmap font for HUD text

**Status:** accepted for the prototype.

The HUD needs on-screen numbers and shop labels before a cache-native font
atlas exists. `crates/client/src/font.rs` defines uppercase ASCII glyphs as
seven 5-bit row masks. `Hud::text` emits one quad per horizontal pixel run with
a one-pixel drop shadow. This is deliberately small and code-only: no Jagex
font assets, no extra dependencies, and no text layout engine. A cache-native
font atlas can replace it later behind the same `Hud::text` call sites.

## ADR-020 — Fountain economy: passive gold + heal + shop range

**Status:** accepted for the prototype.

The player earns 8 gold per second and, within 900 world units of spawn, heals
6% max HP and MP per second. The shop uses the same 900-unit radius, so the
fountain is a real base interaction instead of a global menu. Two persistent
world rings mark the area. The client owns this in the prototype; the server
must own gold, healing, and shop range before multiplayer.

## ADR-021 — Inhibitor respawn and lane super minions

**Status:** accepted.

Destroyed inhibitors respawn after 120 seconds. While an enemy inhibitor in a
lane is down, that lane's next waves include one super minion (2.5x health, 2x
damage, 1.6x scale, slower, gold minimap marker). Nexus vulnerability is still
derived from live inhibitor health, so a respawn re-shields the nexus. This
makes inhibitor pressure meaningful without adding a full super-minion roster
or inhibitor-specific art.

## ADR-022 — Fogged minimap and click-to-pan

**Status:** accepted for the prototype.

The minimap uses the same 192x192 visibility grid as the world fog. Entirely
unseen 8x8-tile blocks are covered by a dark overlay; the block size keeps the
HUD mesh cheap. `minimap_world_at` is a pure screen-to-world conversion, unit
tested for corners/center/outside clicks. A click inside the minimap pans the
camera and disables follow. Fog remains client-side until the authoritative
server owns vision.
