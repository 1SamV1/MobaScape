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

### M1 — Asset pipeline and Karamja carve (in progress)

Scope: sequences/frames, animation playback, sprites/fonts, textures (at least
sampled foliage), asset pack, 3×3 region import, first lane carve, minimap bake.

Done:

- Sequences, frames, framemaps, interpolated CPU vertex animation.
- 3×3 Karamja block with 900 scenery placements and 8 animated NPCs.
- Click-to-move player, selection ring, click marker, camera follow.

Remaining:

- Sprite/font extraction, texture sampling, minimap bake.
- First lane carve, river/bases/tower pads, collision/nav bake.
- Asset pack + reproducible rebuild.
- Missing/unsupported assets fail loudly with cache/model IDs.

### M2 — Playable sandbox (partially started)

Scope: camera/input, click-to-move + hold-steer, A* pathing, one champion,
auto-attacks, one ability, dummy minions, tower, fog v0, HUD v0.

Done so far: click-to-move (direct steering), camera follow/pan, animated
champion, selection ring.

Remaining:

- A* pathfinding + collision + hold-to-steer.
- One real ability and auto-attacks.
- Dummy minions, a tower, fog v0, HUD v0.
- Frame time <16.6 ms at 1080p on the 680M.

### M3 — Match loop (in progress)

Scope: minion waves, jungle camps/buffs, gold/XP/items, five champions, win
condition, dev bots, headless match runner.

Done:

- Three lanes carved into Karamja (top/mid/bottom), two towers and one
  inhibitor per lane per team, one shielded nexus per team.
- Three-minion waves per lane every 25 s (two skeletons, one skeleton mage).
- Tower targeting/projectiles, minion lane AI, inhibitor-gated nexus, win
  condition, victory/defeat HUD border and title.
- Four jungle camps (giant, spider, bat, abyssal demon) with respawn and
  Might/Swift team buffs.
- Live minimap: lanes, structures, camps, minions, player.
- Q/W/E/R champion kit with mana, cooldowns, heal/shield, dash, AoE meteor.
- Levels 1–11, XP, gold, shield, and ability HUD boxes.
- Client-side fog of war: allied vision grid, hidden enemy/neutral units, brush
  zones, and a dark fog mesh over unseen tiles.
- Three data-driven champions: Monkey, JalRek-Jad, Kree'arra Jr.; distinct
  models, stats, and ability numbers; Kree has ranged auto-attacks.
- Six data-driven items, six inventory slots, fountain-range shop, gold
  spending, and stat/ability-power application (`B`, `1`–`6`).
- HUD nexus health bars, wave timer, buff bar, inventory slots, shop rows;
  title shows full match state.
- `--timescale`, `--demo-camp`, `--demo-kit`, `--demo-shop`, and `--champion`
  for verification.

Remaining:

- Server-authoritative fog, minimap click-to-pan, and brush visuals.
- Item selling/combining/passives and five champions with distinct kits.
- HUD text/font atlas so shop names, gold, and stats are on-screen.
- Deterministic 10-bot match at 20× speed with a state hash.
- Data hot reload for champions/abilities/items/waves.
- Balance pass v0.

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
