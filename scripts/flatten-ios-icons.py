#!/usr/bin/env python3
"""Drop the alpha channel from every PNG in a directory.

`tauri icon` writes the iOS app icon set as RGBA even when every pixel is opaque,
and App Store Connect rejects an app icon that has an alpha channel at all. The
phone master is opaque to begin with, so this changes no colour; it only turns
the mode into RGB. A file with a pixel below full alpha is an error, not
something to flatten silently.

    python3 scripts/flatten-ios-icons.py \\
        mobile/src-tauri/gen/apple/Assets.xcassets/AppIcon.appiconset

Run by `npm run icons` in mobile/, right after `tauri icon`. Needs Pillow.
"""

import sys
from pathlib import Path

from PIL import Image


def main():
    folder = Path(sys.argv[1])
    for path in sorted(folder.glob("*.png")):
        img = Image.open(path)
        if img.mode == "RGB":
            continue
        if img.mode != "RGBA" or img.getchannel("A").getextrema()[0] != 255:
            sys.exit(f"{path}: not fully opaque, refusing to flatten")
        img.convert("RGB").save(path)
        print(f"flattened {path}")


if __name__ == "__main__":
    main()
