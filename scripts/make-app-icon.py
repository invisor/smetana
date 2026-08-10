#!/usr/bin/env python3
"""Turn a square artwork into the app icon master that `tauri icon` expands.

macOS does not round an app icon for you, so a full-bleed square sits in the Dock
looking nothing like its neighbours. This writes the shape the system icons
actually have: the squircle body — the Big Sur superellipse, |x|^n + |y|^n = 1 —
filling the 824x824 content square of a 1024 canvas, with the rest transparent.

The artwork is cropped to its own bounding box before it is fitted, because a
source with generous margins of its own would otherwise stack them on top of the
100px the canvas already carries and arrive in the Dock a third too small.

    python3 scripts/make-app-icon.py <source.png> app-icon.png
    npx tauri icon app-icon.png

The About tab draws the same icon and wants the opposite of that margin: it sizes
the visible body itself, in tokens, so a transparent border baked into the file
would be a gap no stylesheet could see. `--body` writes the squircle alone, and
`--size` picks the raster — 256 for a 64px slot, which covers 4x.

    python3 scripts/make-app-icon.py <source.png> src/assets/app-icon.png \
        --body --size 256

Needs Pillow and NumPy, neither of which the app itself depends on — this is run
by hand when the artwork changes, not by the build.
"""

import argparse

from PIL import Image, ImageChops
import numpy as np

CANVAS = 1024
CONTENT = 824    # Apple's content square inside the 1024 canvas
FILL = 0.82      # how much of the squircle the artwork's longest side takes
N = 5.0          # superellipse exponent — the Big Sur corner
SS = 8           # supersampling factor for the mask


def squircle_mask(size, n=N, ss=SS):
    """An antialiased superellipse mask, rendered large and downsampled."""
    big = size * ss
    mask = Image.new("L", (big, big), 0)
    px = mask.load()
    half = big / 2.0
    # For each row solve |y|^n + |x|^n = 1 for x, then fill that span.
    for y in range(big):
        ny = abs((y + 0.5 - half) / half)
        if ny >= 1.0:
            continue
        nx = (1.0 - ny ** n) ** (1.0 / n)
        x0 = int(round(half - nx * half))
        x1 = int(round(half + nx * half))
        for x in range(max(0, x0), min(big, x1)):
            px[x, y] = 255
    return mask.resize((size, size), Image.LANCZOS)


def content_bbox(rgb, tolerance=24):
    """Where the artwork actually is, measured against the corner pixel."""
    a = np.asarray(rgb).astype(int)
    background = a[0, 0]
    ys, xs = np.nonzero(np.abs(a - background).sum(axis=2) > tolerance)
    return int(xs.min()), int(ys.min()), int(xs.max()) + 1, int(ys.max()) + 1


def squircle_body(source, side=CONTENT):
    """The artwork fitted into a squircle of `side`, with nothing around it."""
    src = Image.open(source).convert("RGB")
    background = src.getpixel((0, 0))
    art = src.crop(content_bbox(src))

    scale = side * FILL / max(art.size)
    art = art.resize(
        (round(art.width * scale), round(art.height * scale)), Image.LANCZOS
    )

    body = Image.new("RGBA", (side, side), background + (255,))
    body.paste(art, ((side - art.width) // 2, (side - art.height) // 2))
    body.putalpha(ImageChops.multiply(body.getchannel("A"), squircle_mask(side)))
    return body


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", help="square PNG to build the icon from")
    parser.add_argument("out", nargs="?", default="app-icon.png")
    parser.add_argument(
        "--body",
        action="store_true",
        help="write the squircle alone, without the 1024 canvas margin",
    )
    parser.add_argument("--size", type=int, help="output side in pixels")
    args = parser.parse_args()

    if args.body:
        icon = squircle_body(args.source, args.size or CONTENT)
    else:
        # Build at the full content size and downsample once, so --size never
        # costs the mask its antialiasing.
        icon = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
        inset = (CANVAS - CONTENT) // 2
        body = squircle_body(args.source)
        icon.paste(body, (inset, inset), body)
        if args.size:
            icon = icon.resize((args.size, args.size), Image.LANCZOS)

    icon.save(args.out)
    print(f"wrote {args.out} {icon.size}")


if __name__ == "__main__":
    main()
