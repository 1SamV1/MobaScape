# Roadmap

The roadmap is milestone-based, not date-based. Every milestone must end with a
playable or inspectable artifact and a written verification note. Estimates
assume one full-time developer plus an AI agent; part-time work takes longer.

## Product target (v1)

- 5v5 real-player PvP, server-authoritative.
- Three lanes, jungle camps, towers, inhibitors, nexus, a boss objective.
- 8 champions, 4 abilities + passive each, levels 1–11.
- 6 item slots, gold/XP, fountain shop, recall, respawn timers.
- Fog of war with canopy/brush blockers; no wards/LOS in v1.
- Full lobby: create/join room code, team select, champion select, ready.
- Custom HUD, minimap, scoreboard, shop.
- Native Linux first, then Windows/macOS. Browser is not a v1 target.
- Bots fill dev matches only; production matches require 10 humans.

## Non-goals for v1

Matchmaking service, ranked, progression, skins, spectator/replay, mobile,
browser client, wards, cache music/sound synthesis, player gear compositing,
textures on every surface.

## Milestones

### M0 — Cache spike (current)

Scope: cache reader, terrain/locations/object configs, model types 2/3, wgpu
viewer, tooling.

Exit criteria:

- `cache-info`, `terrain 44 47`, `locations 44 47` work against the pinned cache.
- One Karamja region renders with terrain and 300+ scenery placements.
- One model is exported to OBJ and visually checked.
- Workspace builds with `cargo check`.

Status: **met**, with known limitations in [STATUS.md](STATUS.md).

### M1 — Asset pipeline and Karamja carve

Scope: sequences/frames, animation playback, sprites/fonts, textures (at least
sampled foliage), asset pack, 3×3 region import, first lane carve, minimap bake.

Exit criteria:

- One animated NPC loops correctly in engine space.
- Karamja 3×3 block loads without malformed geometry.
- A 160×160 playable map with three rough lanes, river, bases, and tower pads
  loads from data.
- Asset rebuild is reproducible and under ~2 minutes.
- Missing/unsupported assets fail loudly with cache/model IDs.

### M2 — Playable sandbox

Scope: camera/input, click-to-move + hold-steer, A* pathing, one champion,
auto-attacks, one ability, dummy minions, tower, fog v0, HUD v0.

Exit criteria:

- A single player can move, attack, cast, and die/respawn in the carved map.
- Pathing handles collision and lane corridors without rubber-banding.
- Fog hides enemies behind canopy and at range.
- Frame time <16.6 ms at 1080p on the 680M.

### M3 — Match loop

Scope: minion waves, jungle camps/buffs, gold/XP/items, five champions, win
condition, dev bots, headless match runner.

Exit criteria:

- Deterministic 10-bot match completes at 20× speed with a state hash.
- A human can play a full match against dev bots with all core systems.
- Data hot reload works for champions, abilities, items, and waves.
- Balance pass v0 documented.

### M4 — Multiplayer

Scope: QUIC server, lobby/room codes, prediction/reconciliation, snapshots, fog
filtering, reconnect, match results.

Exit criteria:

- 2 humans + 8 dev bots over LAN at <80 ms RTT.
- 5v5 human match on a hosted server.
- Packet loss/jitter tests pass; no hidden-entity leaks.
- Reconnect within 60 s restores the match.

### M5 — Editor and content

Scope: map editor, procedural terrain/scatter, asset library, second map, 8
champions, audio pass, polish, MCP dev server.

Exit criteria:

- A new map can be authored in the editor in under an hour.
- Eight champions with distinct kits and VFX are playable.
- 5v5 real-player match start-to-finish with lobby and results.
- Dev MCP tools cover screenshot, commands, match state, reload, profile.

## Immediate next tasks

1. Fix/verify model type 1 and old-format support only if needed by the map.
2. Implement sequences/frames and CPU vertex animation for one NPC.
3. Add sprite/font extraction for HUD and item icons.
4. Import a 3×3 Karamja block and build the first lane carve.
5. Add a deterministic `sim` crate with movement + pathing + state hash.
6. Build the editor's minimum carve/place/export loop.

## Creative decisions still open

These require the user's call before implementation:

- **Champion roster:** OSRS bosses as champions (Jad, Zulrah, Vorkath, Kree'arra,
  Callisto, Venenatis, Skotizo, Galvek) vs. player-kit avatars vs. a mix.
- **Map layout:** exact lane geometry, river crossings, boss pit, base theme.
- **Art direction:** fixed isometric camera vs. slight rotation; saturation and
  fog palette; HUD chrome (OSRS parchment vs. clean MOBA).
- **Itemization:** OSRS gear as stat sticks vs. gear with active special attacks.
- **Match length:** target 20–30 minutes vs. a faster 15-minute mode.

## Risks

| Risk | Mitigation |
|---|---|
| Model/animation format drift | Pin cache 235; keep decoder tests; cross-check with RuneLite/osrscachereader |
| XTEA key availability for new caches | Stay on 235 until a full key set exists |
| Map carving damages pathing | Validate connectivity, spawn clearance, and tower reachability in CI |
| Scope explosion | Vertical slices; every milestone playable; non-goals enforced |
| Netcode complexity | Direct-connect first; deterministic sim; packet-loss tests |
| iGPU performance | Chunking, CPU animation first, no shadows/PBR, profile each milestone |
| Asset/IP distribution | No assets in repo; private repo; local extraction only |
