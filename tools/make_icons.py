#!/usr/bin/env python3
"""Generate the AioTrack launcher icons from `icon.png`.

Run from the repository root:

    python3 tools/make_icons.py

The source artwork is the cat-and-piggy-bank illustration committed at the repo
root. Legacy icons keep its warm cream field; the adaptive foreground removes
that field and fits the artwork inside the launcher's circular safe zone.
"""

from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "icon.png"
RES = ROOT / "android" / "res"

# Sampled from the illustration's empty corner.
BACKGROUND = (251, 242, 228)

LEGACY = {
    "mdpi": 48,
    "hdpi": 72,
    "xhdpi": 96,
    "xxhdpi": 144,
    "xxxhdpi": 192,
}
ADAPTIVE = {
    "mdpi": 108,
    "hdpi": 162,
    "xhdpi": 216,
    "xxhdpi": 324,
    "xxxhdpi": 432,
}


def load_source() -> Image.Image:
    if not SOURCE.exists():
        raise SystemExit(f"Missing source artwork: {SOURCE}")
    return Image.open(SOURCE).convert("RGB")


def legacy_icon(source: Image.Image) -> Image.Image:
    size = 1024
    icon = source.resize((size, size), Image.LANCZOS).convert("RGBA")
    # A gentle rounded square; the launcher may mask it further.
    from PIL import ImageDraw

    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        [0, 0, size - 1, size - 1], radius=int(size * 0.22), fill=255
    )
    icon.putalpha(mask)
    return icon


def adaptive_foreground(source: Image.Image) -> Image.Image:
    source = source.resize((1024, 1024), Image.LANCZOS)
    pixels = source.load()
    out = Image.new("RGBA", source.size, (0, 0, 0, 0))
    target = out.load()

    for y in range(source.height):
        for x in range(source.width):
            r, g, b = pixels[x, y]
            distance = max(
                abs(r - BACKGROUND[0]),
                abs(g - BACKGROUND[1]),
                abs(b - BACKGROUND[2]),
            )
            # Soft ramp avoids a jagged halo around the watercolour edges.
            ramp = min(1.0, max(0.0, (distance - 12) / 26.0))
            alpha = int(ramp * 255)
            target[x, y] = (r, g, b, alpha)

    alpha_box = out.getchannel("A").point(lambda a: 255 if a > 40 else 0).getbbox()
    if alpha_box:
        # Keep breathing room around the illustration; a tight crop makes the
        # artwork look zoomed in on the launcher.
        width = alpha_box[2] - alpha_box[0]
        height = alpha_box[3] - alpha_box[1]
        pad = int(max(width, height) * 0.12)
        padded = (
            max(0, alpha_box[0] - pad),
            max(0, alpha_box[1] - pad),
            min(1024, alpha_box[2] + pad),
            min(1024, alpha_box[3] + pad),
        )
        out = out.crop(padded)

    canvas = Image.new("RGBA", (1024, 1024), (0, 0, 0, 0))
    safe = 1024 * 0.54
    scale = min(safe / out.width, safe / out.height)
    resized = out.resize(
        (max(1, int(out.width * scale)), max(1, int(out.height * scale))),
        Image.LANCZOS,
    )
    canvas.alpha_composite(
        resized, ((1024 - resized.width) // 2, (1024 - resized.height) // 2)
    )
    return canvas


def main() -> None:
    source = load_source()
    legacy = legacy_icon(source)
    adaptive = adaptive_foreground(source)

    for density, px in LEGACY.items():
        folder = RES / f"mipmap-{density}"
        folder.mkdir(parents=True, exist_ok=True)
        resized = legacy.resize((px, px), Image.LANCZOS)
        resized.save(folder / "ic_launcher.png")
        resized.save(folder / "ic_launcher_round.png")
        print(f"legacy {density}: {px}px")

    for density, px in ADAPTIVE.items():
        folder = RES / f"mipmap-{density}"
        folder.mkdir(parents=True, exist_ok=True)
        adaptive.resize((px, px), Image.LANCZOS).save(
            folder / "ic_launcher_foreground.png"
        )
        print(f"adaptive {density}: {px}px")

    anydpi = RES / "mipmap-anydpi-v26"
    anydpi.mkdir(parents=True, exist_ok=True)
    icon_xml = """<?xml version="1.0" encoding="utf-8"?>
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@color/ic_launcher_background" />
    <foreground android:drawable="@mipmap/ic_launcher_foreground" />
</adaptive-icon>
"""
    for name in ("ic_launcher.xml", "ic_launcher_round.xml"):
        (anydpi / name).write_text(icon_xml)
    print("adaptive XML written")


if __name__ == "__main__":
    main()
