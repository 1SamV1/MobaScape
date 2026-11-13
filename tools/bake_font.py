#!/usr/bin/env python3
"""Bake the RuneScape interface font into a UI atlas + metrics JSON.

The OSRS client ships its interface font as binary font data outside the
cache, so this uses the RuneScape fonts from the RuneLite resources
(BSD-2-Clause). Drop `runescape.ttf` (and optionally the bold/small variants)
into `assets/osrs-ui/font/` and run this script to regenerate `font.png` and
`font.json`. Both files live in the gitignored asset tree.

Usage:
    python3 tools/bake_font.py [--font assets/osrs-ui/font/runescape.ttf]
                               [--size 16]
"""
import argparse
import json
from pathlib import Path

from PIL import Image, ImageFont

ATLAS_WIDTH = 512
PAD = 1
ALPHA_CUTOFF = 110


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--font", default="assets/osrs-ui/font/runescape.ttf")
    parser.add_argument("--size", type=int, default=16)
    parser.add_argument("--out", default="assets/osrs-ui/font")
    args = parser.parse_args()

    font = ImageFont.truetype(args.font, args.size)
    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)

    chars = [chr(c) for c in range(32, 127)]
    positions = {}
    x, y, row_h = PAD, PAD, 0
    for ch in chars:
        bbox = font.getbbox(ch)
        left, top, right, bottom = bbox
        w, h = max(0, right - left), max(0, bottom - top)
        advance = font.getlength(ch)
        if x + w + PAD > ATLAS_WIDTH:
            x = PAD
            y += row_h + PAD
            row_h = 0
        positions[ch] = (x, y, w, h, left, top, advance)
        x += w + PAD
        row_h = max(row_h, h)

    atlas_h = 1 << (y + row_h + PAD - 1).bit_length()
    atlas = Image.new("RGBA", (ATLAS_WIDTH, atlas_h), (0, 0, 0, 0))
    for ch, (gx, gy, w, h, _left, _top, _advance) in positions.items():
        if w == 0 or h == 0:
            continue
        mask = font.getmask(ch, mode="L")
        mw, mh = mask.size
        glyph = Image.new("RGBA", (mw, mh), (0, 0, 0, 0))
        pixels = glyph.load()
        for yy in range(mh):
            for xx in range(mw):
                if mask.getpixel((xx, yy)) > ALPHA_CUTOFF:
                    pixels[xx, yy] = (255, 255, 255, 255)
        atlas.paste(glyph, (gx, gy))

    atlas.save(out_dir / "font.png")
    data = {
        "size": args.size,
        "atlas_width": ATLAS_WIDTH,
        "atlas_height": atlas_h,
        "glyphs": {
            ch: {
                "x": gx,
                "y": gy,
                "w": w,
                "h": h,
                "left": left,
                "top": top,
                "advance": round(advance, 2),
            }
            for ch, (gx, gy, w, h, left, top, advance) in positions.items()
        },
    }
    (out_dir / "font.json").write_text(json.dumps(data, indent=1))
    print(f"baked {len(positions)} glyphs into {out_dir}/font.png ({atlas.size})")


if __name__ == "__main__":
    main()
