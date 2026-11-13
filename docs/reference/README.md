# Java-era reference (archived)

The 2006Scape Java total-conversion attempt was abandoned when the project pivoted
to a Rust engine using modern OSRS cache assets. These files are kept only as
reference for camera math, HUD layout, and the TCP dev-console pattern.

- `java-moba-tracked.patch` — full diff of tracked Java changes
- `java/MobaCamera.java` — working scene-local camera math
- `java/MobaClient.java`, `java/MobaHud.java` — client mode and HUD
- `java/MobaDevClient.java` — TCP console / deterministic input injection
- `java/xmoba.py`, `java/cc.sh` — X11 screenshot + client console helpers
- `java/server-moba/` — login/arena bootstrap and server dev console

Nothing here is built or used by the Rust workspace.
