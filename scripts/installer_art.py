"""The two pictures the NSIS installer shows, drawn from the brand SVGs.

    python scripts/installer_art.py

Writes `src-tauri/icons/installer-header.bmp` (150x57, the strip at the top of
every page) and `installer-sidebar.bmp` (164x314, the first and the last
page); `tauri.conf.json` points the bundler at both. Run it again when the
mark in `public/Yaca` changes: NSIS takes BMP only, so they cannot be the SVGs
themselves.

Needs Pillow and nothing else. The SVGs are rasterised by hand rather than by
a converter, which is one more thing to install: they use only M/L/H/V/C/Q/Z
paths, circles and rounded rects, and that much fits in this file.
"""

import re
import xml.etree.ElementTree as ET

from pathlib import Path

from PIL import Image, ImageChops, ImageDraw

SS = 8  # supersampling
NS = "{http://www.w3.org/2000/svg}"


def tokens(d):
    return re.findall(r"[MLHVCQZ]|-?\d*\.?\d+(?:e-?\d+)?", d)


def subpaths(d, steps=24):
    """Flattened subpaths of an absolute-command path."""
    out, cur, pos, start = [], [], (0.0, 0.0), (0.0, 0.0)
    tk = tokens(d)
    i, cmd = 0, None

    def num():
        nonlocal i
        v = float(tk[i])
        i += 1
        return v

    while i < len(tk):
        if tk[i].isalpha():
            cmd = tk[i]
            i += 1
            if cmd == "Z":
                if cur:
                    out.append(cur)
                cur, pos = [], start
                continue
        if cmd == "M":
            if cur:
                out.append(cur)
            pos = (num(), num())
            start, cur = pos, [pos]
            cmd = "L"
        elif cmd == "L":
            pos = (num(), num())
            cur.append(pos)
        elif cmd == "H":
            pos = (num(), pos[1])
            cur.append(pos)
        elif cmd == "V":
            pos = (pos[0], num())
            cur.append(pos)
        elif cmd == "C":
            p1, p2, p3 = (num(), num()), (num(), num()), (num(), num())
            for s in range(1, steps + 1):
                t = s / steps
                u = 1 - t
                cur.append((
                    u**3 * pos[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t**3 * p3[0],
                    u**3 * pos[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t**3 * p3[1],
                ))
            pos = p3
        elif cmd == "Q":
            p1, p2 = (num(), num()), (num(), num())
            for s in range(1, steps + 1):
                t = s / steps
                u = 1 - t
                cur.append((
                    u * u * pos[0] + 2 * u * t * p1[0] + t * t * p2[0],
                    u * u * pos[1] + 2 * u * t * p1[1] + t * t * p2[1],
                ))
            pos = p2
        else:
            raise ValueError(cmd)
    if cur:
        out.append(cur)
    return out


def transform_of(el):
    """(tx, ty, scale) of `translate(x y) scale(s)`, the only form used."""
    t = el.get("transform", "")
    m = re.match(r"translate\(([-\d.]+)[ ,]+([-\d.]+)\)\s*scale\(([-\d.]+)\)", t)
    return (float(m[1]), float(m[2]), float(m[3])) if m else (0.0, 0.0, 1.0)


def render(svg_path, height, only=None, recolor=None):
    """RGBA image of the SVG at `height` px. `only(el)` filters elements;
    `recolor(fill)` maps colours."""
    root = ET.parse(svg_path).getroot()
    vx, vy, vw, vh = map(float, root.get("viewBox").split())
    k = height * SS / vh
    w, h = round(vw * k), round(vh * k)
    img = Image.new("RGBA", (w, h), (0, 0, 0, 0))

    for el in root.iter():
        tag = el.tag.replace(NS, "")
        if tag not in ("path", "circle", "rect") or (only and not only(el)):
            continue
        fill = el.get("fill")
        if recolor:
            fill = recolor(fill)
        tx, ty, s = transform_of(el)
        px = lambda x, y: ((tx + x * s - vx) * k, (ty + y * s - vy) * k)
        mask = Image.new("L", (w, h), 0)
        if tag == "path":
            # Even-odd across subpaths: the counters of the letters are holes.
            for sub in subpaths(el.get("d")):
                one = Image.new("L", (w, h), 0)
                ImageDraw.Draw(one).polygon([px(x, y) for x, y in sub], fill=255)
                mask = ImageChops.difference(mask, one)
        elif tag == "circle":
            cx, cy, r = float(el.get("cx")), float(el.get("cy")), float(el.get("r"))
            a, b = px(cx - r, cy - r), px(cx + r, cy + r)
            ImageDraw.Draw(mask).ellipse([a, b], fill=255)
        else:
            x, y = float(el.get("x")), float(el.get("y"))
            rw, rh, rx = float(el.get("width")), float(el.get("height")), float(el.get("rx", 0))
            a, b = px(x, y), px(x + rw, y + rh)
            ImageDraw.Draw(mask).rounded_rectangle([a, b], radius=rx * s * k, fill=255)
        img.paste(Image.new("RGBA", (w, h), fill), (0, 0), mask)

    return img.resize((round(w / SS), round(h / SS)), Image.LANCZOS)


def crop(img):
    return img.crop(img.getbbox())


def main(brand, out):
    is_word = lambda el: el.get("transform") is not None

    # Header: 150x57 on the wizard's white header. The whole lockup, ink on
    # white, set toward the right edge where NSIS puts the image.
    header = Image.new("RGB", (150, 57), "#FFFFFF")
    lockup = crop(render(f"{brand}/yaca-lockup.svg", 30))
    header.paste(lockup, (150 - lockup.width - 12, (57 - lockup.height) // 2), lockup)
    header.save(f"{out}/installer-header.bmp", "BMP")

    # Sidebar: 164x314, the app icon's dark ground with the light mark and the
    # name under it.
    side = Image.new("RGB", (164, 314), "#1B2024")
    mark = crop(render(f"{brand}/yaca-app-icon.svg", 150, only=lambda el: el.get("width") != "64"))
    word = crop(render(f"{brand}/yaca-lockup-dark.svg", 44, only=is_word))
    top = 96
    side.paste(mark, ((164 - mark.width) // 2, top), mark)
    side.paste(word, ((164 - word.width) // 2, top + mark.height + 22), word)
    side.save(f"{out}/installer-sidebar.bmp", "BMP")


if __name__ == "__main__":
    root = Path(__file__).resolve().parent.parent
    main(root / "public" / "Yaca", root / "src-tauri" / "icons")
