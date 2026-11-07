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

# 16x22 wizard, placed on a 32x32 grid. '.' is transparent.
WIZARD = [
    "..............G.",
    "..........H..GGG",
    ".........HH...G.",
    "........HHH...T.",
    ".......HHHH...T.",
    "......hHHHH...T.",
    "......hHsHH...T.",
    ".....hHsssHH..T.",
    ".....hHHsHHH..T.",
    "...BBBBBBBBBBBT.",
    ".....FFFFFF...T.",
    ".....FeFFeF...T.",
    ".....WWWWWW...T.",
    "....RWWWWWWR..T.",
    "...RRWWWWWWRRFT.",
    "...RRRWWWWRRRRT.",
    "..RRRRRWWRRRRRT.",
    "..RRRRRRRRRRRRT.",
    "..DRRRRRRRRRRDT.",
    "..DDRRRRRRRRDDT.",
    "...DDDDDDDDDD.T.",
    "....KK....KK..T.",
]
PAL = {
    "H": (108, 82, 220), "h": (78, 58, 176), "s": (255, 214, 90), "B": (88, 66, 196),
    "F": (242, 202, 166), "e": (28, 20, 40), "W": (238, 238, 246), "R": (72, 58, 182),
    "D": (48, 38, 130), "K": (58, 40, 36), "T": (146, 96, 52), "G": (200, 250, 255),
}
N = 27  # scene grid size
WX, WY = 5, 1  # wizard top-left on the grid
STAFF = (WX + 14 + 0.5, WY + 0.5)  # glowing tip


def h(x, y):
    v = (x * 374761393 + y * 668265263) & 0xFFFFFFFF
    v = ((v ^ (v >> 13)) * 1274126177) & 0xFFFFFFFF
    return v ^ (v >> 16)


def mix(a, b, t):
    t = max(0.0, min(1.0, t))
    return tuple(a[i] + (b[i] - a[i]) * t for i in range(3))


def cell(gx, gy):
    """Colour of one cell of the 32x32 pixel-art scene."""
    # Wizard on top.
    lx, ly = gx - WX, gy - WY
    if 0 <= ly < len(WIZARD) and 0 <= lx < 16:
        ch = WIZARD[ly][lx]
        if ch != ".":
            return PAL[ch]
    # Ground.
    if gy == 23:
        return (106, 186, 80) if h(gx, 1) % 5 else (132, 208, 96)
    if gy == 24:
        return (84, 150, 64) if h(gx, 2) % 3 == 0 else (118, 82, 54)
    if gy > 24:
        return (96, 66, 44) if h(gx, gy) % 4 == 0 else (116, 80, 54)
    # Night sky, a small moon off to the side, a few stars.
    t = gy / 23.0
    sky = mix((16, 13, 40), (70, 42, 110), t)
    d = math.hypot(gx + 0.5 - 5.0, gy + 0.5 - 5.5)
    if d < 3.2:
        return (246, 234, 202)
    if d < 5.5:
        sky = mix(sky, (120, 96, 160), (1 - (d - 3.2) / 2.3) * 0.35)
    if h(gx * 7 + 3, gy * 3 + 1) % 23 == 0 and gy < 18 and d > 5:
        return (226, 224, 255)
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
            g = math.hypot(gx - STAFF[0], gy - STAFF[1])
            if g < 4:
                c = mix(c, (190, 245, 255), (1 - g / 4) ** 2 * 0.5)
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
