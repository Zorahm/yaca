"""The Android launcher icon, drawn from the brand SVGs.

    python scripts/android_icon.py

Writes the layers of an adaptive icon and a manifest for `tauri icon` into
`src-tauri/icons/android-source/`:

    npx tauri icon src-tauri/icons/android-source/manifest.json --output /tmp/yaca-icons
    cp -r /tmp/yaca-icons/android/. src-tauri/icons/android/

`--output` keeps the desktop icons in `src-tauri/icons` as they are: only the
`android/` resources of that run are wanted. `scripts/android-icons.sh` puts
them into the Android project.

An adaptive icon is three pictures the system composes and masks itself: a
background, a foreground with the mark, and a monochrome silhouette that
Android 13 and later recolours to the user's theme. The mark must stay inside
the middle 66 % of the foreground: the launcher crops the rest to a circle, a
squircle or whatever shape the phone has chosen.

Needs Pillow; the SVG is rasterised by `installer_art.render`.
"""

import json
from pathlib import Path

from PIL import Image

from installer_art import render

SIZE = 1024  # the layers are 108 dp; 1024 px is more than any launcher asks for
INK = "#1B2024"
PAPER = "#E7EBE9"
TEAL = "#3FB3AF"
# The mark's height as a share of the layer: inside the 66 % safe zone with a
# margin for the arms that reach the corners of its box.
MARK = 0.62


def mark_layer(recolor):
    """The mark, centred on a transparent square layer."""
    svg = Path(__file__).resolve().parent.parent / "public" / "Yaca" / "yaca-mark.svg"
    # Rendered from the 48-unit mark box, so the layer is the box and the mark
    # sits in it as drawn.
    mark = render(svg, round(SIZE * MARK), recolor=recolor)
    layer = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    layer.paste(mark, ((SIZE - mark.width) // 2, (SIZE - mark.height) // 2), mark)
    return layer


def main(out):
    out.mkdir(parents=True, exist_ok=True)

    # Foreground: the light mark with its teal core, as in the dark app icon.
    def colours(fill):
        return TEAL if fill == "#0F6B6A" else PAPER

    mark_layer(colours).save(out / "foreground.png")

    # Monochrome: alpha is all the system reads. The core is cut out, so the
    # silhouette keeps the detail the colour gave the full-colour mark.
    mono = mark_layer(lambda fill: "#00000000" if fill == "#0F6B6A" else "#000000FF")
    mono.save(out / "monochrome.png")

    # Background: flat ink, given to the manifest as a colour (`bg_color`) so
    # the XML carries no bitmap. `default` feeds the other platforms' icons,
    # which this run does not keep.
    full = Image.new("RGBA", (SIZE, SIZE), INK)
    full.paste(mark_layer(colours), (0, 0), mark_layer(colours))
    full.save(out / "default.png")

    (out / "manifest.json").write_text(
        json.dumps(
            {
                "default": "default.png",
                "bg_color": INK,
                "android_fg": "foreground.png",
                "android_fg_scale": 100,
                "android_monochrome": "monochrome.png",
            },
            indent=2,
        )
        + "\n"
    )


if __name__ == "__main__":
    root = Path(__file__).resolve().parent.parent
    main(root / "src-tauri" / "icons" / "android-source")
