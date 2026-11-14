#!/usr/bin/env python3
import argparse
import json
import pathlib

import numpy as np
from PIL import Image, ImageChops, ImageDraw, ImageEnhance, ImageFilter

ROOT = pathlib.Path(__file__).resolve().parent.parent
PLATES = ROOT / "src" / "assets" / "backdrop"
OUT = ROOT / "src-tauri" / "icons" / "source" / "app-icon.png"
FULL = OUT.with_name("app-icon-full.png")
BLEED = 12

HOUR = 21.0
HOUR_ID = "2100"
PLATE_W, PLATE_H = 2560, 1440

SIZE = 1024
INSET = 100
BODY = SIZE - INSET * 2
RADIUS = 186

MARK_SCALE = 10.3
MARK_OFFSET = 182.4
INK = [(0xFF, 0xF9, 0xEE), (0xF6, 0xE1, 0xE6), (0xDC, 0xD3, 0xFF)]
HALO = [(0xF6, 0xD3, 0xB0), (0xE7, 0xA3, 0xBD), (0xAB, 0x9C, 0xF2)]
MOONLIGHT = np.array([255, 238, 208], dtype=np.float32)

def moon_position(meta):
    arc = meta["moon"]
    hour = HOUR if HOUR >= arc["rise"] else HOUR + 24
    progress = (hour - arc["rise"]) / (arc["set"] - arc["rise"])
    lift = np.sin(np.pi * progress)
    x = PLATE_W * (arc["x0"] + (arc["x1"] - arc["x0"]) * progress)
    y = PLATE_H * (meta["horizon"] - lift ** arc["shape"] * arc["peak"])
    return float(x), float(y)

def scene():
    meta = json.loads((PLATES / "lake.json").read_text(encoding="utf-8"))
    rng = np.random.default_rng(1227)
    horizon = round(PLATE_H * meta["horizon"])
    moon_x, moon_y = moon_position(meta)
    moon_size = 112

    canvas = np.zeros((PLATE_H, PLATE_W, 3), dtype=np.float32)
    canvas[:] = (6, 8, 20)
    sky = Image.open(PLATES / f"lake-{HOUR_ID}-sky.webp").convert("RGB")
    sky = sky.resize((PLATE_W, round(PLATE_H * meta["skyRows"])), Image.LANCZOS)
    canvas[: sky.height] = np.asarray(sky, dtype=np.float32)

    count = 1500
    xs = rng.integers(0, PLATE_W, count)
    ys = (rng.random(count) ** 1.25 * (horizon - 6)).astype(int)
    points = np.zeros((PLATE_H, PLATE_W), dtype=np.float32)
    np.maximum.at(points, (ys, xs), 0.16 + 0.84 * rng.random(count) ** 9)
    points = Image.fromarray(np.uint8(points * 255))
    core = np.asarray(points.filter(ImageFilter.GaussianBlur(0.75)), dtype=np.float32) / 255
    bloom = np.asarray(points.filter(ImageFilter.GaussianBlur(2.2)), dtype=np.float32) / 255
    rows = np.arange(PLATE_H, dtype=np.float32)[:, None]
    cols = np.arange(PLATE_W, dtype=np.float32)[None, :]
    distance = np.hypot(cols - moon_x, rows - moon_y)
    visible = np.clip((horizon - rows) / (horizon * 0.3), 0, 1) * np.clip((distance - moon_size) / (moon_size * 3), 0, 1)
    light = np.clip(core * 2.6 + bloom * 7.0, 0, 1) * visible
    canvas += light[..., None] * np.array([232, 238, 255], dtype=np.float32)

    halo = 0.42 * np.exp(-((distance / (moon_size * 1.5)) ** 2)) + 0.16 * np.exp(-((distance / (moon_size * 5.5)) ** 2))
    canvas += halo[..., None] * MOONLIGHT * (rows < horizon + 4)[..., None]
    image = Image.fromarray(np.uint8(np.clip(canvas, 0, 255))).convert("RGBA")
    moon = Image.open(PLATES / "moon.webp").convert("RGBA").resize((moon_size, moon_size), Image.LANCZOS)
    image.alpha_composite(moon, (round(moon_x - moon_size / 2), round(moon_y - moon_size / 2)))

    for layer in (0, 1):
        clouds = Image.open(PLATES / f"lake-{HOUR_ID}-clouds-{layer}.webp").convert("RGBA")
        image.alpha_composite(clouds.resize((PLATE_W, round(PLATE_H * meta["cloudRows"])), Image.LANCZOS), (0, 0))

    top = round(PLATE_H * meta["landTop"])
    land = Image.open(PLATES / f"lake-{HOUR_ID}-land.webp").convert("RGBA")
    image.alpha_composite(land.resize((PLATE_W, PLATE_H - top), Image.LANCZOS), (0, top))

    canvas = np.asarray(image.convert("RGB"), dtype=np.float32)
    depth = np.clip((rows - horizon) / (PLATE_H * 0.3), 0, 1)
    width = 14 + 170 * depth
    path = np.exp(-(((cols - moon_x) / width) ** 2)) * (rows > horizon + 2)
    strength = 0.95 * np.exp(-2.6 * depth) + 0.1
    sparkle = rng.random((PLATE_H, PLATE_W)).astype(np.float32) ** 18
    sparkle = Image.fromarray(np.uint8(np.clip(sparkle * 255 * 6, 0, 255)))
    sparkle = np.asarray(sparkle.filter(ImageFilter.BoxBlur(1)).resize((PLATE_W, PLATE_H)), dtype=np.float32) / 255
    stretched = np.asarray(
        Image.fromarray(np.uint8(sparkle * 255)).resize((PLATE_W // 3, PLATE_H), Image.BILINEAR).resize((PLATE_W, PLATE_H), Image.BILINEAR),
        dtype=np.float32,
    ) / 255
    glitter = path * strength * (0.24 + 2.7 * stretched)
    canvas += np.clip(glitter, 0, 1.2)[..., None] * MOONLIGHT
    return Image.fromarray(np.uint8(np.clip(canvas, 0, 255))), (moon_x, moon_y), horizon

def realistic_art():
    image, (moon_x, moon_y), _ = scene()
    side = round(PLATE_H * 0.52)
    left = min(max(round(moon_x - side * 0.8), 0), PLATE_W - side)
    top = min(max(round(moon_y - side * 0.25), 0), PLATE_H - side)
    art = image.crop((left, top, left + side, top + side)).resize((BODY, BODY), Image.LANCZOS)
    art = ImageEnhance.Contrast(art).enhance(1.08)
    art = ImageEnhance.Color(art).enhance(1.12)
    grid = np.linspace(-1, 1, BODY, dtype=np.float32)
    shade = 1 - 0.3 * np.clip(np.hypot(grid[None, :], grid[:, None]) - 0.55, 0, 1)
    return Image.fromarray(np.uint8(np.asarray(art, dtype=np.float32) * shade[..., None]))

def ramp(stops, t):
    t = np.asarray(t, dtype=np.float32)
    out = np.zeros(t.shape + (3,), dtype=np.float32)
    for (p0, c0), (p1, c1) in zip(stops, stops[1:]):
        k = np.clip((t - p0) / (p1 - p0), 0, 1)[..., None]
        inside = ((t >= p0) & (t <= p1))[..., None]
        out = np.where(inside, np.array(c0, np.float32) * (1 - k) + np.array(c1, np.float32) * k, out)
    return out

def ridge(seed, width, base, height, detail):
    rng = np.random.default_rng(seed)
    x = np.linspace(0, 1, width, dtype=np.float32)
    y = np.zeros(width, dtype=np.float32)
    for octave in range(detail):
        frequency = 1.3 * 1.9**octave
        y += np.sin(x * frequency * 2 * np.pi + rng.random() * 6.28) / 1.7**octave
    y = (y - y.min()) / (y.max() - y.min())
    return base - height * y**1.35

def stylized_art():
    n = BODY * 2
    rng = np.random.default_rng(1227)
    horizon = round(n * 0.635)
    rows = np.arange(n, dtype=np.float32)[:, None]
    cols = np.arange(n, dtype=np.float32)[None, :]
    moon = (n * 0.775, n * 0.215, n * 0.088)

    sky = ramp(
        [(0.0, (9, 12, 44)), (0.34, (24, 30, 92)), (0.66, (58, 52, 124)), (0.88, (128, 92, 146)), (1.0, (236, 170, 146))],
        np.clip(rows / horizon, 0, 1),
    )
    water = ramp(
        [(0.0, (112, 86, 140)), (0.1, (52, 48, 112)), (0.42, (24, 28, 78)), (1.0, (8, 10, 30))],
        np.clip((rows - horizon) / (n - horizon), 0, 1),
    )
    canvas = np.where((rows < horizon)[..., None], sky, water) * np.ones((n, n, 1), dtype=np.float32)

    distance = np.hypot(cols - moon[0], rows - moon[1])
    glow = 0.5 * np.exp(-((distance / (moon[2] * 2.1)) ** 2)) + 0.2 * np.exp(-((distance / (moon[2] * 6)) ** 2))
    canvas += glow[..., None] * MOONLIGHT * (rows < horizon)[..., None]
    image = Image.fromarray(np.uint8(np.clip(canvas, 0, 255))).convert("RGBA")

    def layer():
        return Image.new("RGBA", (n, n), (0, 0, 0, 0))

    stars = layer()
    draw = ImageDraw.Draw(stars)
    placed = 0
    while placed < 54:
        x, y = rng.random() * n, rng.random() ** 1.4 * horizon * 0.86
        if np.hypot(x - moon[0], y - moon[1]) < moon[2] * 2.4:
            continue
        placed += 1
        fade = 1 - y / horizon
        if placed <= 7:
            arm = n * (0.011 + 0.01 * rng.random())
            for dx, dy in ((arm, arm * 0.16), (arm * 0.16, arm)):
                draw.polygon([(x - dx, y), (x, y - dy), (x + dx, y), (x, y + dy)], fill=(255, 250, 236, 235))
        else:
            r = n * (0.0016 + 0.0026 * rng.random() ** 2)
            draw.ellipse([x - r, y - r, x + r, y + r], fill=(240, 242, 255, int(110 + 140 * fade * rng.random())))
    image.alpha_composite(stars.filter(ImageFilter.GaussianBlur(n * 0.004)))
    image.alpha_composite(stars)

    disc = layer()
    draw = ImageDraw.Draw(disc)
    mx, my, mr = moon
    draw.ellipse([mx - mr, my - mr, mx + mr, my + mr], fill=(255, 250, 232, 255))
    shade = layer()
    ImageDraw.Draw(shade).ellipse([mx - mr * 0.72, my - mr * 0.6, mx + mr * 1.5, my + mr * 1.6], fill=(226, 200, 168, 84))
    shade = shade.filter(ImageFilter.GaussianBlur(mr * 0.34))
    craters = layer()
    draw = ImageDraw.Draw(craters)
    for cx, cy, cr in ((-0.3, -0.26, 0.24), (0.3, 0.22, 0.2), (0.14, -0.42, 0.11), (-0.38, 0.34, 0.12), (0.5, -0.1, 0.09)):
        draw.ellipse([mx + (cx - cr) * mr, my + (cy - cr) * mr, mx + (cx + cr) * mr, my + (cy + cr) * mr], fill=(214, 190, 160, 92))
    craters = craters.filter(ImageFilter.GaussianBlur(mr * 0.045))
    cut = Image.new("L", (n, n), 0)
    ImageDraw.Draw(cut).ellipse([mx - mr, my - mr, mx + mr, my + mr], fill=255)
    for extra in (shade, craters):
        extra.putalpha(ImageChops.multiply(extra.getchannel("A"), cut))
        disc.alpha_composite(extra)
    image.alpha_composite(disc)

    for cy, x0, x1, thickness, color in (
        (0.34, -0.08, 0.5, 0.03, (120, 116, 190, 105)),
        (0.415, 0.46, 1.08, 0.026, (178, 150, 196, 120)),
        (0.5, 0.1, 0.72, 0.018, (150, 122, 178, 90)),
    ):
        cloud = layer()
        draw = ImageDraw.Draw(cloud)
        for i in range(9):
            t = i / 8
            w = n * thickness * (0.75 + 1.9 * np.sin(np.pi * t) + 0.5 * rng.random())
            x = n * (x0 + (x1 - x0) * t)
            y = n * (cy + 0.008 * np.sin(t * 9))
            draw.ellipse([x - w * 3.2, y - w * 0.62, x + w * 3.2, y + w * 0.62], fill=color)
        image.alpha_composite(cloud.filter(ImageFilter.GaussianBlur(n * 0.012)))

    for seed, base, height, detail, color, mirror in (
        (11, 0.0, 0.2, 3, (86, 78, 150), 0.36),
        (23, 0.0, 0.135, 4, (52, 48, 112), 0.5),
        (37, 0.0, 0.075, 4, (22, 24, 66), 0.62),
    ):
        line = ridge(seed, n, horizon + base * n, height * n, detail)
        hill = layer()
        ImageDraw.Draw(hill).polygon([(0, horizon)] + [(x, float(line[x])) for x in range(0, n, 2)] + [(n, horizon)], fill=color + (255,))
        image.alpha_composite(hill)
        below = layer()
        flipped = [(x, float(horizon + (horizon - line[x]) * 0.62)) for x in range(0, n, 2)]
        ImageDraw.Draw(below).polygon([(0, horizon)] + flipped + [(n, horizon)], fill=tuple(int(c * 0.62) for c in color) + (int(255 * mirror),))
        image.alpha_composite(below.filter(ImageFilter.GaussianBlur(n * 0.006)))

    haze = layer()
    ImageDraw.Draw(haze).rectangle([0, horizon - n * 0.03, n, horizon + n * 0.016], fill=(240, 186, 170, 92))
    image.alpha_composite(haze.filter(ImageFilter.GaussianBlur(n * 0.022)))

    canvas = np.asarray(image.convert("RGB"), dtype=np.float32)
    depth = np.clip((rows - horizon) / (n - horizon), 0, 1)
    column = np.exp(-(((cols - moon[0]) / (n * (0.03 + 0.1 * depth))) ** 2)) * (rows > horizon) * (0.5 * np.exp(-2.4 * depth) + 0.05)
    canvas += column[..., None] * MOONLIGHT
    image = Image.fromarray(np.uint8(np.clip(canvas, 0, 255))).convert("RGBA")
    ripples = layer()
    draw = ImageDraw.Draw(ripples)
    for i in range(15):
        t = (i + 0.3) / 15
        y = horizon + (n - horizon) * (0.03 + 0.8 * t**1.7)
        w = n * (0.022 + 0.07 * t) * (0.45 + 1.0 * rng.random())
        h = n * (0.0028 + 0.0046 * t)
        x = moon[0] + n * (0.012 + 0.03 * t) * (rng.random() * 2 - 1)
        draw.ellipse([x - w, y - h, x + w, y + h], fill=(255, 240, 214, int(225 * (1 - 0.74 * t))))
        if rng.random() < 0.6:
            side = x + (w + n * 0.02 * (0.5 + rng.random())) * (1 if rng.random() < 0.5 else -1)
            draw.ellipse([side - w * 0.3, y - h * 0.7, side + w * 0.3, y + h * 0.7], fill=(255, 240, 214, int(150 * (1 - 0.7 * t))))
    image.alpha_composite(ripples.filter(ImageFilter.GaussianBlur(n * 0.0022)))

    lines = layer()
    draw = ImageDraw.Draw(lines)
    for _ in range(16):
        t = rng.random()
        y = horizon + (n - horizon) * (0.1 + 0.85 * t)
        x = rng.random() * n
        w = n * (0.02 + 0.05 * t)
        draw.rounded_rectangle([x - w, y - n * 0.0012, x + w, y + n * 0.0012], radius=n * 0.0012, fill=(190, 186, 236, int(40 + 50 * t)))
    image.alpha_composite(lines)

    art = image.convert("RGB").resize((BODY, BODY), Image.LANCZOS)
    grid = np.linspace(-1, 1, BODY, dtype=np.float32)
    shade = 1 - 0.26 * np.clip(np.hypot(grid[None, :], grid[:, None]) - 0.6, 0, 1)
    return Image.fromarray(np.uint8(np.asarray(art, dtype=np.float32) * shade[..., None]))

def rounded(inset):
    mask = Image.new("L", (SIZE * 2, SIZE * 2), 0)
    box = [(INSET + inset) * 2, (INSET + inset) * 2, (SIZE - INSET - inset) * 2, (SIZE - INSET - inset) * 2]
    ImageDraw.Draw(mask).rounded_rectangle(box, radius=(RADIUS - inset) * 2, fill=255)
    return mask.resize((SIZE, SIZE), Image.LANCZOS)

def stroke(width):
    scale = MARK_SCALE * 2
    offset = MARK_OFFSET * 2
    radius = width * scale / 2
    mask = Image.new("L", (SIZE * 2, SIZE * 2), 0)
    draw = ImageDraw.Draw(mask)

    def stamp(points):
        for x, y in points:
            px, py = offset + x * scale, offset + y * scale
            draw.ellipse([px - radius, py - radius, px + radius, py + radius], fill=255)

    steps = np.linspace(0, 1, 420)
    stamp([(20, 17.5 + 29 * t) for t in steps])
    stamp([(24.5 + 7.5 * t, 13) for t in steps])
    stamp([(32 + 11 * np.sin(a), 24 - 11 * np.cos(a)) for a in np.linspace(0, np.pi, 520)])
    stamp([(32 - 12 * t, 35) for t in steps])
    p0, p1, p2, p3 = (26, 35), (33.5, 35), (37.5, 40.5), (41.4, 47.3)
    stamp(
        [
            tuple((1 - t) ** 3 * p0[i] + 3 * (1 - t) ** 2 * t * p1[i] + 3 * (1 - t) * t**2 * p2[i] + t**3 * p3[i] for i in range(2))
            for t in steps
        ]
    )
    for cx, cy in ((20, 13), (20, 51), (44, 51)):
        stamp([(cx + 4.5 * np.cos(a), cy + 4.5 * np.sin(a)) for a in np.linspace(0, 2 * np.pi, 420)])
    return mask.resize((SIZE, SIZE), Image.LANCZOS)

def gradient(stops):
    start = np.array([MARK_OFFSET + 13 * MARK_SCALE, MARK_OFFSET + 7 * MARK_SCALE], dtype=np.float32)
    end = np.array([MARK_OFFSET + 51 * MARK_SCALE, MARK_OFFSET + 57 * MARK_SCALE], dtype=np.float32)
    axis = end - start
    ys, xs = np.mgrid[0:SIZE, 0:SIZE].astype(np.float32)
    t = np.clip(((xs - start[0]) * axis[0] + (ys - start[1]) * axis[1]) / float(axis @ axis), 0, 1)[..., None]
    a, b, c = (np.array(stop, dtype=np.float32) for stop in stops)
    color = np.where(t < 0.5, a + (b - a) * (t * 2), b + (c - b) * (t * 2 - 1))
    return Image.fromarray(np.uint8(color)).convert("RGBA")

def tinted(color, mask, opacity=1.0):
    layer = color.copy()
    layer.putalpha(mask.point(lambda value: round(value * opacity)))
    return layer

def main():
    parser = argparse.ArgumentParser(description="Draw the app icon.")
    parser.add_argument("--style", choices=["stylized", "realistic"], default="stylized")
    style = parser.parse_args().style
    body = rounded(0)
    blank = (0, 0, 0, 0)

    plate = Image.new("RGBA", (SIZE, SIZE), blank)
    plate.paste(stylized_art() if style == "stylized" else realistic_art(), (INSET, INSET))
    plate.putalpha(body)

    ring = ImageChops.subtract(body, rounded(6))
    fall = np.interp(np.linspace(0, 1, SIZE), [0, INSET / SIZE, 0.42, 1 - INSET / SIZE, 1], [0.34, 0.34, 0.07, 0.03, 0.03])
    edge = np.asarray(ring, dtype=np.float32) * fall[:, None]

    bed = stroke(7.4).filter(ImageFilter.GaussianBlur(30))
    bed = Image.composite(bed, Image.new("L", (SIZE, SIZE), 0), body)
    glow = stroke(5.8).filter(ImageFilter.GaussianBlur(MARK_SCALE * 2.3))
    layers = [
        plate,
        tinted(Image.new("RGBA", (SIZE, SIZE), (255, 255, 255, 255)), Image.fromarray(np.uint8(edge))),
        tinted(Image.new("RGBA", (SIZE, SIZE), (8, 10, 26, 255)), bed, 0.5),
        tinted(gradient(HALO), glow, 0.78),
        tinted(gradient(INK), stroke(4.4)),
    ]

    shadow = ImageChops.offset(body, 0, 14).filter(ImageFilter.GaussianBlur(18))
    icon = Image.new("RGBA", (SIZE, SIZE), blank)
    icon.alpha_composite(tinted(Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 255)), shadow, 0.34))
    full = Image.new("RGBA", (SIZE, SIZE), blank)
    for layer in layers:
        icon.alpha_composite(layer)
        full.alpha_composite(layer)

    OUT.parent.mkdir(parents=True, exist_ok=True)
    icon.save(OUT, optimize=True)
    print(OUT)
    near = INSET - BLEED
    full.crop((near, near, SIZE - near, SIZE - near)).resize((SIZE, SIZE), Image.LANCZOS).save(FULL, optimize=True)
    print(FULL)

if __name__ == "__main__":
    main()
