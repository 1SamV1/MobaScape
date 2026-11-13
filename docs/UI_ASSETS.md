# OSRS UI asset pipeline

The client can render authentic OSRS item icons and UI tab/button sprites from
the gitignored `assets/osrs-ui` tree. Assets are **never committed**.

## 1. Sprite package

Download the weekly OSRS sprite package (1× and 8× PNGs) from
[skelspanels.com/sprites](https://skelspanels.com/sprites). The current tested
package is `sprites-2026-11-11`.

```sh
cd /home/sam/Desktop/2006Scape/rift
mkdir -p assets/osrs-sprites assets/osrs-ui/items assets/osrs-ui/ui
curl -L --fail -o /tmp/osrs-sprites.zip \
  https://github.com/skeldoor/osrs-sprites/releases/download/sprites-2026-11-11/sprites-2026-11-11.zip
unzip -q -o /tmp/osrs-sprites.zip -d assets/osrs-sprites
```

The pack is named `<archive>_<frame>.png`, e.g. `5841_0.png`.

## 2. UI sprites used by the shop/HUD

```sh
SPRITES=assets/osrs-sprites/sprites-2026-11-11/1x
cp "$SPRITES/5841_0.png" assets/osrs-ui/ui/tab.png
cp "$SPRITES/655_0.png"  assets/osrs-ui/ui/button.png
cp "$SPRITES/656_0.png"  assets/osrs-ui/ui/button_red.png
cp "$SPRITES/8339_0.png" assets/osrs-ui/ui/header.png
```

The client currently uses `tab.png` for the category/header/buy/sell/shop
buttons. The other sprites are available for future panels.

## 3. Item icons

Download the exact item icons from the OSRS Wiki (with a descriptive
User-Agent; the wiki rejects default clients):

```sh
python3 - <<'PY'
import urllib.request, urllib.parse
from pathlib import Path
items = {
    "iron_plate": "Iron_platebody.png",
    "fury": "Amulet_of_fury.png",
    "berserker": "Berserker_ring.png",
    "ahrims": "Ahrim's_staff.png",
    "boots": "Dragon_boots.png",
}
out = Path("assets/osrs-ui/items")
for key, name in items.items():
    url = "https://oldschool.runescape.wiki/images/" + urllib.parse.quote(name)
    req = urllib.request.Request(url, headers={"User-Agent": "MobaScape/0.1"})
    (out / f"{key}.png").write_bytes(urllib.request.urlopen(req, timeout=30).read())
PY
```

The bronze sword inventory icon is not available under the plain wiki file
name. The client uses the `Bronze_shortsword_detail.png` render, trimmed and
fit to the icon atlas:

```sh
python3 - <<'PY'
import urllib.request
from pathlib import Path
url = "https://oldschool.runescape.wiki/images/Bronze_shortsword_detail.png"
req = urllib.request.Request(url, headers={"User-Agent": "MobaScape/0.1"})
Path("assets/osrs-ui/items/bronze_sword.png").write_bytes(
    urllib.request.urlopen(req, timeout=30).read()
)
PY
```

## 4. How the client loads it

`crates/client/src/ui_assets.rs` builds a 1024×1024 RGBA atlas at startup:

- item icons are trimmed of transparent margins and fit to 128×128;
- UI sprites keep their native size;
- a single white texel lets untextured HUD quads share the textured pipeline;
- missing files are skipped and the HUD falls back to procedural icons.

The atlas is uploaded as `Rgba8UnormSrgb` and sampled with nearest filtering so
the OSRS pixel art stays crisp. `crates/client/src/ui.wgsl` is the textured HUD
pipeline.

## 5. Rules

- Never commit `assets/` or extracted cache assets.
- Keep the asset source, package date, and sprite IDs together in notes.
- Verify one icon and one tab visually before batching replacements.
- Do not add a sprite decoder to the repo just to avoid downloading the pack;
  the external PNG path is deliberate and keeps the repository asset-free.
