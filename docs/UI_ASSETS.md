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


## Interface font and HUD icons

The OSRS interface font is not stored in the cache: the client ships it as
binary font data, and RuneLite redistributes it as `runescape.ttf`,
`runescape_bold.ttf` and `runescape_small.ttf` (BSD-2-Clause). Put the regular
font in `assets/osrs-ui/font/runescape.ttf` and bake it:

```sh
python3 tools/bake_font.py
```

This writes `font.png` (a 512px-wide white glyph atlas) and `font.json`
(per-glyph rects, bearings and advances) next to the font. Both are gitignored.
`ui_assets.rs` copies the atlas into the UI atlas at `(2, 700)` and the HUD
draws text with whole-pixel glyph scales so nearest filtering keeps the pixel
font crisp. If the font files are missing, the HUD falls back to the built-in
5x7 bitmap font.

HUD icons are scraped from the OSRS sprite dump into `assets/osrs-ui/ui/`:

| key | sprite | use |
| --- | --- | --- |
| `panel` | 1017 | nine-slice interface panel for every HUD window |
| `icon_hp` | 119 | hitpoints heart |
| `icon_mp` | 1099 | mana orb |
| `icon_gold` | 1089 | coins |
| `icon_xp` | 1196 | experience |
| `icon_swords` | 1073 | kills |
| `icon_skull` | 1096 | deaths |
| `icon_q` | 127 | ability Q (wand) |
| `icon_w` | 1098 | ability W (orb) |
| `icon_e` | 129 | ability E (green orb) |
| `icon_r` | 1060 | ability R (red orb) |

The existing `tab`, `button`, `button_red` and `header` sprites plus the six
item icons are loaded the same way. Missing files fall back to procedural
shapes so a clean checkout still runs.
