# Netcode (planned)

This document is the target design for v1. None of it is implemented yet; the
current code is a single-process viewer.

## Principles

- **Server-authoritative.** The server owns movement validation, damage,
  cooldowns, mana, vision, gold, item purchases, and match results.
- **Deterministic sim.** `crates/sim` is IO-free, fixed-timestep, seeded RNG.
  The server and client run the same movement code for prediction.
- **Fog is a server rule.** The server never sends an entity a team cannot see.
  Client-side fog is presentation, not security.
- **Bots are dev-only.** A production match requires 10 human clients.

## Tick rates

| Thing | Rate |
|---|---|
| Simulation | 30 Hz (33.333 ms) |
| Client input | 30 Hz (one command per tick, with client tick + last ack) |
| Snapshots | 20 Hz (every 1.5 ticks; send on tick 0 and 2 of a 3-tick cycle) |
| Remote interpolation | 100 ms buffer |
| Lag compensation rewind | up to 250 ms |
| Visibility grid | 10 Hz |

## Input

Client sends `InputCommand { client_tick, ack_tick, move_mode, target, cast }`.

- `move_mode`: `None`, `PathTo(point)`, `Steer(direction)` (hold-to-steer), or
  `Attack(target_id)`.
- `cast`: ability slot, cursor point/target, shift/self flag.
- Inputs are rate-limited and validated (range, cooldown, mana, cast time,
  visibility, silence/stun).
- The server stores the last N inputs for reconciliation/lag comp.

## Movement

- Tap-to-move: server A* on the tile grid (`crates/sim` already owns the grid and
  pathfinder); returns a path. Client predicts along the path and reconciles on
  ack.
- Minions, towers, nexus health, wave timers, and win conditions must move into
  the authoritative `sim`; the current lane loop is a client-side prototype.
- Hold-to-steer: client sends a direction each tick; server applies collision
  sliding at the same speed and validates against the map.
- Server correction: if the client's predicted position differs by more than a
  threshold, send a `CorrectPosition` with the authoritative state and the last
  processed input tick.
- Remote units are interpolated; do not extrapolate through walls.

## Snapshots

- Baseline snapshot + delta-compressed subsequent snapshots.
- Entity state: id, kind, team, position, facing, hp/mana, level, animation,
  status flags, ability cooldowns (own team only for enemies? cooldowns hidden),
  item slots (own team only), buffs, shield.
- Snapshot filtering: only entities visible to the receiving team; hidden
  entities are omitted entirely (no positions, no health).
- Quantize positions to 1/8 tile (16 units), angles to 1/256 turn, health to
  integers. Use bitcode/postcard; measure size before optimizing.

## Abilities and projectiles

- Cast is server-validated at the cast tick; the client plays the animation
  immediately and shows a pending effect.
- Projectiles are server-simulated; hit detection rewinds targets up to 250 ms
  and checks the server's hitbox, not the client's.
- On confirm, clients receive a `CastConfirmed`/`CastRejected` event; on reject,
  refund mana/cooldown and play a cancel animation.
- Ground-targeted AoE uses the server's cursor position, clamped to range.

## Transport

- **QUIC** via `quinn` + `rustls` for game traffic (reliable streams for
  control, unreliable datagrams for snapshots/inputs).
- Direct-connect v1: the host runs the server and shares a room code containing
  `host:port` and the TLS certificate fingerprint. No central matchmaking.
- Lobby/control: a small TCP/TLS or QUIC stream for create/join, team select,
  champion select, ready, and match start.
- Room code format: `MOBA-<base32(ip:port:fingerprint)>`; the client pins the
  fingerprint from the code.
- LAN discovery via mDNS is optional; do not block v1 on it.

## Lobby and match lifecycle

1. Host creates a room; server generates a room code.
2. Clients join, authenticate with a display name + session token (v1 can be
   ephemeral; no passwords).
3. Team select (5 per team), champion select with lock-in and timeout.
4. Ready check; 10 humans required unless `--dev-bots N` is set.
5. Server loads the map/asset hash; clients with a different hash are rejected.
6. Match runs; results are written to a local SQLite file (optional v1).
7. Reconnect: a client has 60 s to rejoin with the same session token; the
   server restores its champion state and fog.

## Anti-cheat / validation

- Never trust client positions, cooldowns, gold, damage, or visibility.
- Rate-limit inputs; reject impossible movement deltas.
- Server-side fog filtering is the primary anti-wallhack measure.
- Log suspicious input patterns (too many casts, impossible path speed) for dev
  review; do not auto-ban in v1.

## Tests

- In-process server + two clients with a simulated link: 0/50/150 ms RTT, 0/5%
  loss, 0/30 ms jitter.
- Assert reconciliation converges within 200 ms and never tunnels through walls.
- Assert hidden entities never appear in snapshots.
- Golden state hashes for deterministic sim runs.
- Fuzz the snapshot decoder; reject malformed/oversized messages.

## Open questions

- Exact snapshot cadence vs. bandwidth for 10 players + 80 units.
- Whether to use one QUIC stream per match or per team.
- Whether the lobby service is separate from the match server.
- How much ability state is public (cooldowns, mana) vs. hidden.
