#!/usr/bin/env python3
"""Draw the Falling Wizard app icon from code (no image editor, no libraries).

Writes:
  assets/icon.png            1024x1024 master
  assets/AppIcon.icns        macOS app icon (via iconutil)
  assets/icon_{16,32,64}.rgba raw RGBA for the game window
  web/img/icon.png, web/img/favicon.png
"""
import math, os, struct, subprocess, tempfile, zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# The wizard exactly as he appears in the game (player.rs), facing right.
SPRITE = [".h...", ".hh..", ".hsh.", "hhhhh", ".ffe.", ".www.", "rrwrr", ".rrr.", ".rrr.", ".b.b."]
PAL = {
    "h": (92, 70, 196), "s": (255, 220, 110), "f": (236, 196, 160), "e": (30, 20, 30),
    "w": (230, 230, 236), "r": (64, 50, 160), "b": (60, 44, 40),
}
N = 16          # scene grid
WX, WY = 5, 3   # sprite top-left
# Staff from the hand up and to the right, like in game, with a glowing tip.
STAFF = [(9, 7), (10, 6), (11, 5)]
TIP = (12, 4)
GLOW = (12.5, 4.5)


def h(x, y):
    v = (x * 374761393 + y * 668265263) & 0xFFFFFFFF
    v = ((v ^ (v >> 13)) * 1274126177) & 0xFFFFFFFF
    return v ^ (v >> 16)


def mix(a, b, t):
    t = max(0.0, min(1.0, t))
    return tuple(a[i] + (b[i] - a[i]) * t for i in range(3))


def shade(c, gx, gy, amt=0.18):
    f = 1.0 + ((h(gx, gy) % 256) / 255.0 - 0.5) * amt
    return tuple(min(255, v * f) for v in c)


def cell(gx, gy):
    """One cell of the scene: the in-game wizard on a grassy hill under the game's sky."""
    lx, ly = gx - WX, gy - WY
    if 0 <= ly < len(SPRITE) and 0 <= lx < 5 and SPRITE[ly][lx] != ".":
        return PAL[SPRITE[ly][lx]]
    if (gx, gy) in STAFF:
        return (130, 86, 44)
    if (gx, gy) == TIP:
        return (170, 235, 255)
    if gy == 13:
        return shade((78, 156, 62), gx, gy, 0.3)
    if gy == 14:
        return shade((112, 78, 52), gx, gy, 0.35)
    if gy > 14:
        return shade((98, 92, 102), gx, gy, 0.45)
    if gy == 12 and gx in (1, 13, 14):
        return (72, 164, 62)  # grass tufts
    # The game's daytime sky, with one small cloud.
    sky = mix((62, 106, 178), (182, 210, 230), gy / 12.0)
    if (gy == 2 and 1 <= gx <= 3) or (gy == 1 and gx == 2):
        return mix(sky, (244, 246, 250), 0.85)
    return sky


def render(size):
    """Render the icon at `size` px: macOS-style rounded square with the scene inside."""
    m = size * 100 / 1024
    rect = size - 2 * m
    radius = rect * 0.225
    unit = rect / N
    px = bytearray()
    for y in range(size):
        px.append(0)  # PNG filter byte
        for x in range(size):
            # Rounded-rect coverage with a soft edge.
            cx = min(max(x + 0.5, m + radius), size - m - radius)
            cy = min(max(y + 0.5, m + radius), size - m - radius)
            dist = math.hypot(x + 0.5 - cx, y + 0.5 - cy) - radius
            a = max(0.0, min(1.0, 0.5 - dist))
            if a <= 0:
                px += b"\0\0\0\0"
                continue
            gx, gy = (x - m) / unit, (y - m) / unit
            c = cell(min(N - 1, max(0, int(gx))), min(N - 1, max(0, int(gy))))
            # Soft glow from the staff tip.
            g = math.hypot(gx - GLOW[0], gy - GLOW[1])
            if g < 2.2:
                c = mix(c, (200, 245, 255), (1 - g / 2.2) ** 2 * 0.5)
            px += bytes((int(c[0]), int(c[1]), int(c[2]), int(a * 255)))
    return bytes(px)


def png(path, size, data):
    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    raw = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
    raw += chunk(b"IDAT", zlib.compress(data, 9)) + chunk(b"IEND", b"")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(raw)


def strip_filters(data, size):
    out = bytearray()
    for y in range(size):
        out += data[y * (size * 4 + 1) + 1:(y + 1) * (size * 4 + 1)]
    return bytes(out)


def main():
    cache = {}
    def get(sz):
        if sz not in cache:
            cache[sz] = render(sz)
        return cache[sz]

    png(os.path.join(ROOT, "assets/icon.png"), 1024, get(1024))
    png(os.path.join(ROOT, "web/img/icon.png"), 256, get(256))
    png(os.path.join(ROOT, "web/img/favicon.png"), 64, get(64))
    for sz in (16, 32, 64):
        with open(os.path.join(ROOT, f"assets/icon_{sz}.rgba"), "wb") as f:
            f.write(strip_filters(get(sz), sz))
    with tempfile.TemporaryDirectory() as tmp:
        iconset = os.path.join(tmp, "AppIcon.iconset")
        os.makedirs(iconset)
        for base in (16, 32, 128, 256, 512):
            png(os.path.join(iconset, f"icon_{base}x{base}.png"), base, get(base))
            png(os.path.join(iconset, f"icon_{base}x{base}@2x.png"), base * 2, get(base * 2))
        subprocess.run(["iconutil", "-c", "icns", iconset, "-o", os.path.join(ROOT, "assets/AppIcon.icns")], check=True)
    print("icon written")


if __name__ == "__main__":
    main()
