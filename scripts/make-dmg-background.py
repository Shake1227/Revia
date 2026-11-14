#!/usr/bin/env python3
import math
import pathlib
import random
import shutil
import subprocess

from PIL import Image, ImageDraw, ImageFilter

WIDTH, HEIGHT = 660, 400
SCALE = 4
HORIZON = 252
ICON_Y = 150
OUT = pathlib.Path(__file__).resolve().parent.parent / "src-tauri" / "icons" / "dmg"

SKY = [
    (0.00, (13, 14, 38)),
    (0.30, (32, 29, 76)),
    (0.46, (72, 57, 121)),
    (0.545, (124, 96, 155)),
    (0.59, (148, 116, 167)),
    (0.63, (176, 140, 180)),
]
WATER = [
    (0.63, (168, 136, 178)),
    (0.70, (112, 92, 150)),
    (0.82, (52, 46, 100)),
    (1.00, (18, 18, 48)),
]

def lerp(a, b, t):
    return tuple(round(a[i] + (b[i] - a[i]) * t) for i in range(3))

def gradient(stops, y):
    t = y / (HEIGHT * SCALE)
    for (t0, c0), (t1, c1) in zip(stops, stops[1:]):
        if t0 <= t <= t1:
            return lerp(c0, c1, (t - t0) / (t1 - t0))
    return stops[-1][1] if t > stops[-1][0] else stops[0][1]

def ridge(rng, base, amplitude, roughness=0.55, steps=9):
    points = [0.0, 0.0]
    spread = 1.0
    for _ in range(steps):
        next_points = []
        for left, right in zip(points, points[1:]):
            next_points += [left, (left + right) / 2 + rng.uniform(-spread, spread)]
        next_points.append(points[-1])
        points = next_points
        spread *= roughness
    low, high = min(points), max(points)
    span = (high - low) or 1
    return [base - (value - low) / span * amplitude for value in points]

def main():
    rng = random.Random(27)
    w, h = WIDTH * SCALE, HEIGHT * SCALE
    horizon = HORIZON * SCALE

    image = Image.new("RGB", (w, h))
    draw = ImageDraw.Draw(image)
    for y in range(h):
        draw.line([(0, y), (w, y)], fill=gradient(SKY if y < horizon else WATER, y))
    image = image.convert("RGBA")

    stars = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    star_draw = ImageDraw.Draw(stars)
    for _ in range(230):
        x = rng.uniform(0, w)
        y = rng.uniform(0, horizon * 0.72) ** 1.0
        fade = 1 - y / (horizon * 0.78)
        radius = rng.choice([0.5, 0.6, 0.7, 0.9, 1.2]) * SCALE * 0.55
        alpha = int(rng.uniform(70, 235) * max(fade, 0.12))
        star_draw.ellipse([x - radius, y - radius, x + radius, y + radius], fill=(244, 238, 255, alpha))
    image.alpha_composite(stars.filter(ImageFilter.GaussianBlur(SCALE * 0.18)))

    glow = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    glow_draw = ImageDraw.Draw(glow)
    moon_x, moon_y, moon_r = 596 * SCALE, 52 * SCALE, 15 * SCALE
    for ring, alpha in ((5.2, 18), (3.4, 30), (2.1, 52)):
        r = moon_r * ring
        glow_draw.ellipse([moon_x - r, moon_y - r, moon_x + r, moon_y + r], fill=(250, 214, 190, alpha))
    glow_draw.ellipse([w * 0.1, horizon - 26 * SCALE, w * 0.9, horizon + 22 * SCALE], fill=(250, 206, 182, 96))
    image.alpha_composite(glow.filter(ImageFilter.GaussianBlur(SCALE * 16)))

    moon = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    moon_draw = ImageDraw.Draw(moon)
    moon_draw.ellipse([moon_x - moon_r, moon_y - moon_r, moon_x + moon_r, moon_y + moon_r], fill=(250, 240, 222, 255))
    craters = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    crater_draw = ImageDraw.Draw(craters)
    for dx, dy, r, alpha in ((-0.3, -0.2, 0.26, 54), (0.28, 0.22, 0.2, 44), (0.02, 0.48, 0.14, 38)):
        cx, cy, cr = moon_x + dx * moon_r, moon_y + dy * moon_r, r * moon_r
        crater_draw.ellipse([cx - cr, cy - cr, cx + cr, cy + cr], fill=(206, 186, 180, alpha))
    moon.alpha_composite(craters.filter(ImageFilter.GaussianBlur(SCALE * 0.9)))
    image.alpha_composite(moon.filter(ImageFilter.GaussianBlur(SCALE * 0.45)))

    layers = [
        (horizon - 0 * SCALE, 30 * SCALE, (112, 90, 152, 205)),
        (horizon + 1 * SCALE, 20 * SCALE, (76, 60, 122, 225)),
        (horizon + 2 * SCALE, 11 * SCALE, (52, 42, 98, 240)),
    ]
    for base, amplitude, color in layers:
        heights = ridge(rng, base, amplitude)
        step = w / (len(heights) - 1)
        outline = [(index * step, y) for index, y in enumerate(heights)]
        layer = Image.new("RGBA", (w, h), (0, 0, 0, 0))
        ImageDraw.Draw(layer).polygon(outline + [(w, horizon + 3 * SCALE), (0, horizon + 3 * SCALE)], fill=color)
        image.alpha_composite(layer.filter(ImageFilter.GaussianBlur(SCALE * 0.4)))
        mirrored = layer.transpose(Image.FLIP_TOP_BOTTOM)
        offset = h - 2 * (horizon + 3 * SCALE)
        reflection = Image.new("RGBA", (w, h), (0, 0, 0, 0))
        reflection.paste(mirrored, (0, -offset))
        reflection = reflection.filter(ImageFilter.GaussianBlur(SCALE * 2.2))
        reflection.putalpha(reflection.getchannel("A").point(lambda value: int(value * 0.34)))
        image.alpha_composite(reflection)

    glints = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    glint_draw = ImageDraw.Draw(glints)
    for _ in range(90):
        y = horizon + rng.uniform(6, 120) ** 1.0 * SCALE
        depth = (y - horizon) / (h - horizon)
        length = rng.uniform(10, 46) * SCALE * (0.5 + depth)
        x = moon_x - 30 * SCALE + rng.gauss(0, 1) * (26 + 120 * depth) * SCALE
        alpha = int(rng.uniform(60, 150) * (1 - depth * 0.55))
        glint_draw.line([(x - length / 2, y), (x + length / 2, y)], fill=(250, 226, 208, alpha), width=max(1, int(SCALE * 0.5)))
    image.alpha_composite(glints.filter(ImageFilter.GaussianBlur(SCALE * 0.5)))

    mist = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    mist_draw = ImageDraw.Draw(mist)
    for _ in range(9):
        cx = rng.uniform(-0.1, 1.1) * w
        cy = horizon + rng.uniform(-10, 18) * SCALE
        rx, ry = rng.uniform(90, 220) * SCALE, rng.uniform(6, 13) * SCALE
        mist_draw.ellipse([cx - rx, cy - ry, cx + rx, cy + ry], fill=(226, 204, 226, 46))
    image.alpha_composite(mist.filter(ImageFilter.GaussianBlur(SCALE * 9)))

    arrow = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    arrow_draw = ImageDraw.Draw(arrow)
    y = ICON_Y * SCALE
    start, end = 292 * SCALE, 366 * SCALE
    stroke = int(SCALE * 1.6)
    color = (244, 236, 252, 150)
    for index in range(7):
        x0 = start + (end - start) * index / 7
        x1 = x0 + (end - start) / 7 * 0.52
        arrow_draw.line([(x0, y), (x1, y)], fill=color, width=stroke)
    head = 9 * SCALE
    arrow_draw.line([(end - head, y - head), (end + SCALE, y), (end - head, y + head)], fill=color, width=stroke, joint="curve")
    image.alpha_composite(arrow.filter(ImageFilter.GaussianBlur(SCALE * 0.3)))

    vignette = Image.new("L", (w, h), 0)
    vignette_draw = ImageDraw.Draw(vignette)
    vignette_draw.ellipse([-w * 0.25, -h * 0.4, w * 1.25, h * 1.35], fill=255)
    vignette = vignette.filter(ImageFilter.GaussianBlur(SCALE * 60))
    shade = Image.new("RGBA", (w, h), (6, 6, 20, 120))
    shade.putalpha(vignette.point(lambda value: int((255 - value) * 0.5)))
    image.alpha_composite(shade)

    noise = Image.effect_noise((w // 2, h // 2), 9).resize((w, h)).convert("L")
    grain = Image.merge("RGBA", (noise, noise, noise, noise.point(lambda _: 10)))
    image.alpha_composite(grain)

    OUT.mkdir(parents=True, exist_ok=True)
    final = image.convert("RGB")
    standard = OUT / "background.png"
    retina = OUT / "background@2x.png"
    final.resize((WIDTH, HEIGHT), Image.LANCZOS).save(standard, dpi=(72, 72))
    final.resize((WIDTH * 2, HEIGHT * 2), Image.LANCZOS).save(retina, dpi=(144, 144))
    if shutil.which("tiffutil"):
        subprocess.run(["tiffutil", "-cathidpicheck", str(standard), str(retina), "-out", str(OUT / "background.tiff")], check=True)
    print(f"wrote {OUT}")

if __name__ == "__main__":
    main()
