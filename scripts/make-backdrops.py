#!/usr/bin/env python3
import argparse
import json
import math
import pathlib

import numpy as np
from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "src" / "assets" / "backdrop"

BASE_W, BASE_H = 2560, 1440
HORIZON = 0.345
FOV = math.radians(50)
SKY_ROWS = 0.40
LAND_TOP = 0.17
CLOUD_ROWS = 0.375

KEY_HOURS = [0.0, 4.5, 5.5, 6.5, 8.0, 12.0, 16.0, 17.5, 18.5, 19.5, 21.0]

SUN = dict(rise=5.3, set=18.7, x0=0.38, x1=0.95, peak=0.245, shape=0.5)
MOON = dict(rise=17.5, set=30.5, x0=0.40, x1=0.95, peak=0.225, shape=0.5)

def elevation(hour):
    day = SUN["set"] - SUN["rise"]
    if SUN["rise"] <= hour <= SUN["set"]:
        return math.sin(math.pi * (hour - SUN["rise"]) / day)
    since_set = (hour - SUN["set"]) % 24.0
    return -math.sin(math.pi * since_set / (24.0 - day))

def body_position(arc, hour):
    h = hour if hour >= arc["rise"] else hour + 24.0
    if not arc["rise"] <= h <= arc["set"]:
        return None
    progress = (h - arc["rise"]) / (arc["set"] - arc["rise"])
    lift = math.sin(math.pi * progress)
    return arc["x0"] + (arc["x1"] - arc["x0"]) * progress, HORIZON - lift ** arc["shape"] * arc["peak"], lift

def srgb(*rgb):
    return (np.array(rgb, dtype=np.float32) / 255.0) ** 2.2

def encode(linear):
    return (np.clip(linear, 0.0, 1.0) ** (1 / 2.2) * 255.0 + 0.5).astype(np.uint8)

def smoothstep(a, b, x):
    t = np.clip((x - a) / (b - a), 0.0, 1.0)
    return t * t * (3.0 - 2.0 * t)

def mix(a, b, t):
    return a + (b - a) * t

def luminance(color):
    return color[..., 0] * 0.2126 + color[..., 1] * 0.7152 + color[..., 2] * 0.0722

def spectral(rng, rows, cols, height, width, beta, fmin=0.0, fmax=None):
    fy = np.fft.fftfreq(rows, d=height / rows)[:, None]
    fx = np.fft.rfftfreq(cols, d=width / cols)[None, :]
    f = np.hypot(fx, fy)
    amp = np.zeros_like(f)
    keep = f > 0
    if fmin:
        keep &= f >= fmin
    if fmax:
        keep &= f <= fmax
    amp[keep] = f[keep] ** (-beta / 2.0)
    phase = rng.uniform(0.0, 2.0 * np.pi, f.shape)
    field = np.fft.irfft2(amp * np.exp(1j * phase), s=(rows, cols))
    field -= field.mean()
    field /= field.std() + 1e-9
    return field.astype(np.float32)

def ridged(rng, rows, cols, height, width, base, octaves):
    total = np.zeros((rows, cols), dtype=np.float32)
    weight = np.ones((rows, cols), dtype=np.float32)
    amp, norm = 1.0, 0.0
    for octave in range(octaves):
        f = base * 2.0**octave
        band = spectral(rng, rows, cols, height, width, 0.0, f * 0.7, f * 1.45)
        crest = np.clip(1.0 - np.abs(band) / 2.3, 0.0, 1.0) ** 2
        crest *= weight
        weight = np.clip(crest * 1.9, 0.0, 1.0)
        total += crest * amp
        norm += amp
        amp *= 0.52
    return total / norm

def blur(field, sigma_y, sigma_x):
    for axis, sigma in ((0, sigma_y), (1, sigma_x)):
        if sigma < 0.5:
            continue
        radius = int(sigma * 2.5)
        kernel = np.exp(-0.5 * (np.arange(-radius, radius + 1) / sigma) ** 2).astype(np.float32)
        kernel /= kernel.sum()
        pad = [(0, 0)] * 3
        pad[axis] = (radius, radius)
        padded = np.pad(field, pad, mode="edge")
        out = np.zeros_like(field)
        size = field.shape[axis]
        for index, k in enumerate(kernel):
            out += (padded[index : index + size] if axis == 0 else padded[:, index : index + size]) * k
        field = out
    return field

class Scene:
    def __init__(self, name, scale, seed):
        self.name = name
        self.seed = seed
        self.rng = np.random.default_rng(seed)
        self.geometry = None
        self.waves = None
        self.w = int(round(BASE_W * scale / 2)) * 2
        self.h = int(round(BASE_H * scale / 2)) * 2
        self.hy = int(round(self.h * HORIZON))
        self.focal = (self.w / 2) / math.tan(FOV / 2)
        self.unit = self.w / BASE_W

    def stream(self, index):
        return np.random.default_rng([self.seed, index])

    def sky_gradient(self, stops, horizon_far, horizon_near, glow_x, glow_width):
        w, hy = self.w, self.hy
        rows = self.h
        t = (np.arange(rows, dtype=np.float32) / hy)[:, None]
        x = (np.arange(w, dtype=np.float32) / w)[None, :]
        glow = np.exp(-(((x - glow_x) / glow_width) ** 2))
        positions = np.array([stop[0] for stop in stops], dtype=np.float32)
        colors = np.stack([stop[1] for stop in stops])
        column = np.stack([np.interp(t[:, 0], positions, colors[:, c]) for c in range(3)], axis=1)
        sky = np.repeat(column[:, None, :], w, axis=1)
        horizon = mix(horizon_far[None, None, :], horizon_near[None, None, :], glow[:, :, None])
        lift = smoothstep(0.62, 1.0, t)[:, :, None] ** 1.6
        sky = mix(sky, horizon, lift * (0.55 + 0.45 * glow[:, :, None]))
        return sky.astype(np.float32), glow[0]

    def stars(self, sky, count, strength, band=None, stream=1):
        w, hy, unit = self.w, self.hy, self.unit
        rng = self.stream(stream)
        rows = sky.shape[0]
        layer = np.zeros((rows, w, 3), dtype=np.float32)
        brightness = luminance(sky)

        def splat(xs, ys, power, tint, sigma):
            radius = max(1, int(math.ceil(sigma * 3)))
            xi, yi = np.floor(xs).astype(int), np.floor(ys).astype(int)
            for dy in range(-radius, radius + 1):
                for dx in range(-radius, radius + 1):
                    px, py = xi + dx, yi + dy
                    ok = (px >= 0) & (px < w) & (py >= 0) & (py < rows)
                    weight = np.exp(-(((px - xs) ** 2 + (py - ys) ** 2) / (2 * sigma * sigma)))
                    np.add.at(layer, (py[ok], px[ok]), (weight * power)[ok, None] * tint[ok])

        def field(n, low, high, sigma):
            xs = rng.uniform(0, w, n)
            ys = rng.uniform(0, rows, n)
            keep = rng.uniform(0, 1, n)
            if band is not None:
                chosen = keep < band[np.clip(ys.astype(int), 0, rows - 1), np.clip(xs.astype(int), 0, w - 1)]
                xs, ys = xs[chosen], ys[chosen]
            power = np.clip(low * rng.uniform(0.02, 1.0, len(xs)) ** -0.62, 0, high)
            warm = rng.uniform(0, 1, len(xs))[:, None]
            tint = mix(np.array([0.72, 0.82, 1.0]), np.array([1.0, 0.86, 0.7]), warm**2.2)
            splat(xs, ys, power, tint.astype(np.float32), sigma * unit)

        field(int(count * unit * unit), 0.02, 0.9, 0.62)
        field(int(count * 0.04 * unit * unit), 0.22, 2.2, 0.95)
        field(int(count * 0.004 * unit * unit) + 3, 0.5, 2.4, 2.4)
        dim = 1.0 - smoothstep(0.012, 0.14, brightness)
        if self.name != "hoshi":
            t = (np.arange(rows, dtype=np.float32) / hy)[:, None]
            dim = dim * (1.0 - smoothstep(0.66, 0.98, t))
        return sky + layer * dim[:, :, None] * strength

    def milky_way(self, sky, start, end, width, strength):
        w = self.w
        rng = self.stream(2)
        rows = sky.shape[0]
        yy, xx = np.mgrid[0:rows, 0:w].astype(np.float32)
        ax, ay = start[0] * w, start[1] * rows
        bx, by = end[0] * w, end[1] * rows
        length = math.hypot(bx - ax, by - ay)
        distance = ((xx - ax) * (by - ay) - (yy - ay) * (bx - ax)) / length
        along = ((xx - ax) * (bx - ax) + (yy - ay) * (by - ay)) / length**2
        profile = np.exp(-((distance / (width * w)) ** 2))
        cloud = spectral(rng, rows, w, rows / w, 1.0, 2.6, fmin=1.5)
        dust = spectral(rng, rows, w, rows / w, 1.0, 2.2, fmin=3.0)
        lane = np.exp(-((distance / (width * w * 0.26) + 0.35 * dust) ** 2))
        density = profile * np.clip(0.62 + 0.3 * cloud, 0.0, 1.3) * (1.0 - 0.62 * lane * smoothstep(-0.4, 0.6, dust))
        density *= smoothstep(-0.25, 0.2, along) * (1.0 - smoothstep(0.85, 1.3, along))
        core = np.exp(-((distance / (width * w * 0.5)) ** 2))
        tint = mix(srgb(120, 138, 210), srgb(226, 196, 176), core[:, :, None] * 0.7)
        dim = 1.0 - smoothstep(0.02, 0.16, luminance(sky))
        glow = density[:, :, None] * tint * 0.055 * strength * dim[:, :, None]
        return sky + glow, np.clip(density * dim, 0.0, 1.0)

    def sun_glow(self, sky, cx, cy, color, strength):
        w = self.w
        rows = sky.shape[0]
        yy, xx = np.mgrid[0:rows, 0:w].astype(np.float32)
        d = np.hypot(xx - cx * w, (yy - cy * self.h) * 1.2) / w
        glow = 0.55 * np.exp(-d / 0.16) + 0.5 * np.exp(-d / 0.05)
        return sky + glow[:, :, None] * color[None, None, :] * strength

    def clouds(self, seed, coverage, band, streak, shadow, lit, edge, alpha, glow, glow_tint):
        w = self.w
        rows = int(round(self.h * CLOUD_ROWS))
        rng = np.random.default_rng(seed)
        source_rows = rows * 2
        height = source_rows / w * streak
        base = spectral(rng, source_rows, w, height, 1.0, 3.6, fmin=1.0)
        detail = spectral(rng, source_rows, w, height, 1.0, 2.6, fmin=7.0)
        noise = base + 0.16 * detail
        t = np.arange(rows, dtype=np.float32) / self.hy
        depth = 1.0 / (1.5 - np.clip(t, 0.0, 1.08))
        source = (depth - depth[0]) / (depth[-1] - depth[0]) * (source_rows - 2)
        low = np.floor(source).astype(int)
        frac = (source - low)[:, None]
        field = noise[low] * (1.0 - frac) + noise[low + 1] * frac
        envelope = np.exp(-(((t - band[0]) / band[1]) ** 2))[:, None]
        cover = smoothstep(coverage, coverage + 1.7, field * (0.5 + 0.5 * envelope) + (envelope - 1.0) * 1.6)
        thin = cover * (1.0 - cover) * 4.0
        warm = (glow[None, :] * smoothstep(0.35, 1.0, t)[:, None])[:, :, None]
        under = mix(lit[None, None, :], glow_tint[None, None, :], warm)
        color = mix(under, shadow[None, None, :], (cover**0.75)[:, :, None])
        color = color + thin[:, :, None] * edge[None, None, :] * 0.5
        fade = (smoothstep(0.0, 0.1, t) * (1.0 - smoothstep(0.97, 1.06, t)))[:, None]
        return np.concatenate([color, (cover * alpha * fade)[:, :, None]], axis=2).astype(np.float32)

    def terrain(self, ranges, ss):
        if self.geometry is not None:
            return self.geometry
        cols = self.w * ss
        depth_rows = 1500
        span_u = 2.0 * math.tan(FOV / 2)
        span_v = math.log(60.0 / 1.2)
        du, dv = span_u / cols, span_v / depth_rows
        u = ((np.arange(cols, dtype=np.float32) + 0.5) / cols - 0.5) * span_u
        v = (np.arange(depth_rows, dtype=np.float32) + 0.5) / depth_rows
        rng = self.stream(4)

        field = np.zeros((depth_rows, cols), dtype=np.float32)
        owner = np.zeros((depth_rows, cols), dtype=np.int8)
        relative = np.zeros((depth_rows, cols), dtype=np.float32)
        for index, spec in enumerate(ranges):
            wall = np.exp(-(((v - spec["v"]) / spec["width"]) ** 2))[:, None]
            texture = ridged(rng, depth_rows, cols, span_v, span_u, spec["grain"], spec["octaves"])
            swell = spectral(rng, 1, cols, 1.0, span_u, 2.0, fmin=0.8, fmax=7.0)[0]
            profile = np.clip(0.62 + 0.3 * swell, 0.2, 1.25)
            for centre, half, depth in spec.get("gaps", []):
                profile = profile * (1.0 - depth * np.exp(-(((u - centre) / half) ** 2)))
            shape = wall * profile[None, :] * (spec["floor"] + (1.0 - spec["floor"]) * texture)
            layer = spec["amp"] * shape
            mask = layer > field
            field = np.where(mask, layer, field)
            owner = np.where(mask, index, owner)
            relative = np.where(mask, shape, relative)

        grad_u = np.gradient(field, du, axis=1)
        grad_v = np.gradient(field, dv, axis=0)
        slope_x = grad_u
        slope_z = field - u[None, :] * grad_u + grad_v
        length = np.sqrt(slope_x**2 + slope_z**2 + 1.0)
        normal = np.stack([-slope_x / length, 1.0 / length, -slope_z / length], axis=2)

        crest = np.maximum.accumulate(field, axis=0)
        rows = int(math.ceil(float(crest.max()) * self.focal * ss)) + 4 * ss
        rows += (-rows) % ss
        alpha_rows = (np.arange(rows, dtype=np.float32)[::-1] + 0.5) / (self.focal * ss)
        hit = np.empty((rows, cols), dtype=np.int32)
        for column in range(cols):
            hit[:, column] = np.searchsorted(crest[:, column], alpha_rows, side="left")
        covered = hit < depth_rows
        hit = np.minimum(hit, depth_rows - 1)
        column_index = np.broadcast_to(np.arange(cols)[None, :], hit.shape)

        n = normal[hit, column_index]
        which = owner[hit, column_index]
        rel = relative[hit, column_index]
        grit = spectral(rng, rows, cols, rows / cols, 1.0, 1.2, fmin=20.0)
        patch = spectral(rng, rows, cols, rows / cols, 1.0, 2.2, fmin=4.0)
        steep = 1.0 - n[:, :, 1]
        snow_line = np.array([spec.get("snow", 9.0) for spec in ranges], dtype=np.float32)[which]
        tree_line = np.array([spec.get("trees", 0.0) for spec in ranges], dtype=np.float32)[which]
        self.geometry = dict(
            ss=ss,
            normal=n,
            grit=grit,
            snow=smoothstep(0.0, 0.16, rel - snow_line + 0.07 * patch) * (1.0 - smoothstep(0.42, 0.72, steep)),
            trees=1.0 - smoothstep(-0.05, 0.1, rel - tree_line + 0.06 * patch),
            distance=v[hit],
            elevation=np.broadcast_to(alpha_rows[:, None], hit.shape),
            base_of=np.array([spec["amp"] for spec in ranges], dtype=np.float32)[which],
            cover=covered.astype(np.float32),
            near=ranges[-1]["v"],
            far=ranges[0]["v"],
        )
        return self.geometry

    def land(self, ranges, palette, haze_row, light, ss=2):
        g = self.terrain(ranges, ss)
        n = g["normal"]
        albedo = mix(palette["rock"][None, None, :], palette["forest"][None, None, :], g["trees"][:, :, None])
        albedo = albedo * (1.0 + 0.16 * g["grit"][:, :, None])
        albedo = mix(albedo, palette["snow"][None, None, :], g["snow"][:, :, None])

        direction = np.array(light["dir"], dtype=np.float32)
        direction /= np.linalg.norm(direction)
        lambert = np.clip(n[:, :, 0] * direction[0] + n[:, :, 1] * direction[1] + n[:, :, 2] * direction[2], 0.0, 1.0)
        sky_fill = 0.3 + 0.7 * n[:, :, 1]
        lit = albedo * (palette["ambient"][None, None, :] * sky_fill[:, :, None] + light["color"][None, None, :] * lambert[:, :, None])

        near, far = g["near"], g["far"]
        aerial = smoothstep(near - 0.12, far + 0.05, g["distance"]) ** light["haze_curve"] * light["haze"]
        mist = np.exp(-g["elevation"] / (g["base_of"] * light["mist_height"])) * light["mist"] * smoothstep(near - 0.1, far, g["distance"] + 0.2)
        fog = np.clip(aerial + mist * (1.0 - aerial), 0.0, 0.985)
        veil = mix(haze_row, np.broadcast_to(light["veil"], haze_row.shape), light["veil_mix"]) * light["haze_gain"]
        haze = np.repeat(veil, ss, axis=0)[None, :, :]
        color = mix(lit, np.broadcast_to(haze, lit.shape), fog[:, :, None])

        cover = g["cover"]
        color = color * cover[:, :, None]
        out_rows = cover.shape[0] // ss
        color = color.reshape(out_rows, ss, self.w, ss, 3).mean(axis=(1, 3))
        cover = cover.reshape(out_rows, ss, self.w, ss).mean(axis=(1, 3))
        return color.astype(np.float32), cover.astype(np.float32)

    def ripples(self):
        if self.waves is not None:
            return self.waves
        w, hy, h = self.w, self.hy, self.h
        rows = h - hy
        yy = (np.arange(rows, dtype=np.float32) + 0.5)[:, None]
        xx = np.arange(w, dtype=np.float32)[None, :]
        camera = 2.2
        z = camera * self.focal / yy
        world_x = (xx - w / 2) * z / self.focal
        rng = self.stream(5)
        slope = np.zeros((rows, w), dtype=np.float32)
        sway = np.zeros((rows, w), dtype=np.float32)
        count = 40
        for _ in range(count):
            length = math.exp(rng.uniform(math.log(0.25), math.log(14.0)))
            angle = rng.normal(0.0, 0.55)
            kx, kz = math.sin(angle) * 2 * math.pi / length, math.cos(angle) * 2 * math.pi / length
            pixels = length * self.focal / (z * z) * camera
            visible = smoothstep(1.5, 5.0, pixels[:, 0])[:, None]
            phase = kx * world_x + kz * z + rng.uniform(0, 2 * math.pi)
            steep = length**-0.15
            slope += visible * steep * math.cos(angle) * np.cos(phase)
            sway += visible * steep * math.sin(angle) * np.cos(phase)
        slope /= count**0.5
        sway /= count**0.5
        reach = 2.0 * self.unit + 0.11 * yy
        sample = np.clip(yy + np.clip(slope * reach, -yy * 0.9, None), 0.5, hy - 1.0)
        source = hy - sample
        low = np.clip(np.floor(source).astype(int), 0, hy - 2)
        level = np.clip(yy / (rows * 0.16), 0.0, 2.999) * np.ones((1, w), dtype=np.float32)
        which = np.floor(level).astype(int)
        grazing = np.clip(yy / self.focal, 0.0, 1.0)
        reflect = np.clip((0.2 + 0.8 * (1.0 - grazing) ** 5.0) * (1.0 - 0.12 * slope), 0.0, 1.0)
        self.waves = dict(
            low=low,
            frac=(source - low)[:, :, None],
            which=which,
            blend=(level - which)[:, :, None],
            columns=np.clip(np.rint(xx + sway * reach * 2.0).astype(int), 0, w - 1),
            reflect=reflect[:, :, None],
        )
        return self.waves

    def lake(self, above, deep, tint):
        waves = self.ripples()
        unit = self.unit
        low, frac, which, columns = waves["low"], waves["frac"], waves["which"], waves["columns"]
        blurred = [above, blur(above, 3.0 * unit, 1.0 * unit), blur(above, 10.0 * unit, 4.0 * unit), blur(above, 28.0 * unit, 14.0 * unit)]
        mirror = np.zeros((self.h - self.hy, self.w, 3), dtype=np.float32)
        for index in range(3):
            pick = which == index
            if not pick.any():
                continue
            a = blurred[index][low, columns] * (1 - frac) + blurred[index][low + 1, columns] * frac
            b = blurred[index + 1][low, columns] * (1 - frac) + blurred[index + 1][low + 1, columns] * frac
            mirror = np.where(pick[:, :, None], a * (1 - waves["blend"]) + b * waves["blend"], mirror)
        reflect = waves["reflect"]
        return (mirror * tint[None, None, :] * reflect + deep[None, None, :] * (1.0 - reflect)).astype(np.float32)

def save_rgb(path, linear, quality=88, width=None):
    image = Image.fromarray(encode(linear))
    if width and width != image.width:
        image = image.resize((width, round(image.height * width / image.width)), Image.LANCZOS)
    image.save(path, "WEBP", quality=quality, method=6)

def save_rgba(path, linear_rgba, quality=86, width=None):
    rgb = encode(linear_rgba[:, :, :3])
    alpha = (np.clip(linear_rgba[:, :, 3], 0.0, 1.0) * 255.0 + 0.5).astype(np.uint8)
    image = Image.fromarray(np.dstack([rgb, alpha]))
    if width and width != image.width:
        image = image.resize((width, round(image.height * width / image.width)), Image.LANCZOS)
    image.save(path, "WEBP", quality=quality, alpha_quality=92, method=6)

def over(base, layer_rgba):
    alpha = layer_rgba[:, :, 3:4]
    return base * (1.0 - alpha) + layer_rgba[:, :, :3] * alpha

RANGES = [
    dict(v=0.88, width=0.075, amp=0.074, grain=4.2, octaves=9, floor=0.26, snow=0.6, trees=0.0, gaps=[(-0.13, 0.11, 0.62), (0.33, 0.085, 0.5)]),
    dict(v=0.64, width=0.07, amp=0.05, grain=6.0, octaves=9, floor=0.24, snow=0.86, trees=0.2, gaps=[(-0.1, 0.16, 0.5), (0.34, 0.07, 0.3)]),
    dict(v=0.4, width=0.065, amp=0.031, grain=8.0, octaves=8, floor=0.3, trees=0.7, gaps=[(0.02, 0.2, 0.45)]),
    dict(v=0.17, width=0.06, amp=0.0135, grain=13.0, octaves=7, floor=0.42, trees=2.0),
]

CLOUD_SHAPES = [
    dict(seed=11, coverage=0.55, band=(0.5, 0.42), streak=5.0),
    dict(seed=29, coverage=0.3, band=(0.84, 0.2), streak=3.2),
]

def look(stops, far, near, stars, milky, clouds, cloud_glow, rock, forest, snow, ambient, light, haze, water, base, css, glow=0.3, sun=0.0):
    positions = (0.0, 0.22, 0.44, 0.62, 0.76, 0.88, 1.0, 1.2)
    return dict(
        stops=np.stack([srgb(*color) for color in stops + [stops[-1]]]),
        positions=positions,
        horizon_far=srgb(*far),
        horizon_near=srgb(*near),
        glow_width=glow,
        stars=stars,
        milky=milky,
        clouds=[dict(shadow=srgb(*c[0]), lit=srgb(*c[1]), edge=srgb(*c[2]), alpha=c[3]) for c in clouds],
        cloud_glow=srgb(*cloud_glow),
        rock=srgb(*rock),
        forest=srgb(*forest),
        snow=srgb(*snow),
        ambient=srgb(*ambient[0]) * ambient[1],
        light=srgb(*light[0]) * light[1],
        haze=haze[0],
        haze_curve=haze[1],
        haze_gain=haze[2],
        veil=srgb(*haze[3]),
        veil_mix=haze[4],
        mist=haze[5],
        mist_height=haze[6],
        deep=srgb(*water[0]),
        tint=np.array(water[1], dtype=np.float32),
        base=srgb(*base),
        sun=sun,
        css={name: np.array(value, dtype=np.float32) for name, value in css.items()},
    )

NIGHT = look(
    stops=[(5, 8, 22), (8, 12, 34), (13, 18, 50), (20, 24, 66), (28, 30, 80), (36, 36, 92), (44, 42, 100)],
    far=(40, 40, 96),
    near=(56, 54, 112),
    stars=1.0,
    milky=0.85,
    clouds=[((20, 22, 48), (64, 70, 120), (140, 150, 220), 0.5), ((22, 22, 50), (72, 74, 124), (150, 156, 220), 0.8)],
    cloud_glow=(90, 96, 150),
    rock=(40, 42, 62),
    forest=(10, 16, 24),
    snow=(124, 134, 180),
    ambient=((100, 112, 190), 1.0),
    light=((190, 204, 255), 2.2),
    haze=(0.7, 0.85, 0.42, (70, 72, 132), 0.7, 0.45, 0.2),
    water=((4, 7, 18), (0.84, 0.87, 0.96)),
    base=(8, 11, 22),
    css=dict(mist=(150, 156, 220, 0.26), ripple=(190, 200, 255, 0.26), halo=(206, 214, 255, 0.2), glint=(250, 244, 228, 0.5)),
)

DUSK = look(
    stops=[(5, 8, 24), (9, 14, 40), (17, 23, 62), (30, 31, 84), (54, 42, 104), (96, 62, 116), (138, 86, 118)],
    far=(112, 78, 122),
    near=(246, 158, 112),
    stars=0.85,
    milky=0.6,
    clouds=[((34, 32, 70), (92, 84, 150), (150, 160, 230), 0.5), ((40, 32, 72), (120, 92, 148), (190, 170, 220), 0.86)],
    cloud_glow=(250, 150, 110),
    rock=(46, 44, 62),
    forest=(14, 20, 26),
    snow=(128, 134, 176),
    ambient=((120, 124, 196), 1.1),
    light=((196, 206, 255), 2.4),
    haze=(0.66, 0.85, 0.5, (96, 86, 150), 0.55, 0.5, 0.2),
    water=((6, 9, 22), (0.86, 0.88, 0.96)),
    base=(10, 13, 25),
    css=dict(mist=(176, 172, 232, 0.34), ripple=(204, 210, 255, 0.3), halo=(214, 220, 255, 0.2), glint=(255, 246, 228, 0.5)),
)

DAWN = look(
    stops=[(8, 12, 34), (14, 20, 52), (26, 32, 76), (46, 48, 100), (76, 68, 122), (122, 96, 138), (170, 128, 146)],
    far=(110, 96, 138),
    near=(236, 160, 140),
    stars=0.6,
    milky=0.3,
    clouds=[((40, 40, 78), (120, 104, 156), (190, 180, 230), 0.5), ((48, 42, 80), (176, 126, 150), (230, 190, 210), 0.84)],
    cloud_glow=(250, 170, 150),
    rock=(52, 52, 74),
    forest=(18, 26, 34),
    snow=(150, 150, 190),
    ambient=((130, 136, 204), 1.15),
    light=((200, 206, 250), 1.6),
    haze=(0.72, 0.8, 0.6, (120, 110, 160), 0.55, 0.7, 0.26),
    water=((10, 14, 30), (0.88, 0.9, 0.97)),
    base=(14, 18, 34),
    css=dict(mist=(206, 196, 236, 0.42), ripple=(220, 216, 250, 0.32), halo=(226, 220, 250, 0.2), glint=(255, 240, 230, 0.5)),
)

SUNSET = look(
    stops=[(28, 40, 96), (44, 58, 126), (78, 80, 152), (128, 98, 160), (190, 116, 150), (236, 138, 120), (250, 164, 110)],
    far=(206, 132, 146),
    near=(255, 176, 96),
    stars=0.1,
    milky=0.0,
    clouds=[((120, 86, 128), (255, 184, 150), (255, 220, 190), 0.56), ((140, 90, 118), (255, 166, 118), (255, 214, 170), 0.86)],
    cloud_glow=(255, 190, 118),
    rock=(74, 66, 86),
    forest=(30, 36, 44),
    snow=(230, 190, 190),
    ambient=((150, 140, 200), 1.15),
    light=((255, 170, 120), 1.5),
    haze=(0.78, 0.8, 0.78, (190, 140, 160), 0.5, 0.55, 0.22),
    water=((26, 32, 66), (0.92, 0.92, 0.97)),
    base=(24, 26, 50),
    css=dict(mist=(255, 210, 200, 0.3), ripple=(255, 226, 210, 0.4), halo=(255, 204, 150, 0.5), glint=(255, 232, 200, 0.8)),
    sun=0.5,
)

SUNRISE = look(
    stops=[(70, 100, 170), (104, 134, 196), (150, 170, 214), (196, 196, 224), (230, 208, 220), (248, 214, 204), (255, 220, 190)],
    far=(224, 206, 220),
    near=(255, 214, 170),
    stars=0.0,
    milky=0.0,
    clouds=[((176, 170, 200), (255, 226, 214), (255, 244, 236), 0.58), ((186, 170, 196), (255, 214, 190), (255, 240, 224), 0.84)],
    cloud_glow=(255, 220, 180),
    rock=(96, 100, 128),
    forest=(44, 60, 70),
    snow=(255, 236, 230),
    ambient=((190, 196, 236), 1.2),
    light=((255, 206, 170), 1.1),
    haze=(0.88, 0.75, 0.92, (214, 206, 226), 0.6, 0.85, 0.32),
    water=((120, 140, 180), (0.96, 0.97, 1.0)),
    base=(190, 200, 222),
    css=dict(mist=(255, 255, 255, 0.7), ripple=(255, 255, 255, 0.6), halo=(255, 232, 200, 0.5), glint=(255, 246, 226, 0.85)),
    sun=0.45,
)

GOLDEN = look(
    stops=[(70, 116, 196), (100, 144, 210), (140, 172, 222), (180, 196, 228), (214, 212, 220), (240, 216, 190), (252, 218, 170)],
    far=(230, 216, 206),
    near=(255, 224, 160),
    stars=0.0,
    milky=0.0,
    clouds=[((176, 168, 186), (255, 240, 214), (255, 250, 236), 0.58), ((184, 170, 180), (255, 232, 196), (255, 246, 226), 0.84)],
    cloud_glow=(255, 226, 170),
    rock=(112, 110, 124),
    forest=(54, 72, 66),
    snow=(255, 246, 230),
    ambient=((190, 200, 232), 1.2),
    light=((255, 222, 170), 1.25),
    haze=(0.84, 0.75, 0.96, (220, 206, 206), 0.5, 0.35, 0.24),
    water=((70, 104, 150), (0.96, 0.97, 1.0)),
    base=(186, 198, 220),
    css=dict(mist=(255, 246, 230, 0.5), ripple=(255, 250, 236, 0.6), halo=(255, 236, 190, 0.55), glint=(255, 250, 230, 0.9)),
    sun=0.4,
)

MORNING = look(
    stops=[(170, 196, 232), (190, 210, 236), (214, 222, 238), (236, 228, 232), (248, 226, 214), (252, 224, 204), (252, 224, 204)],
    far=(232, 226, 234),
    near=(255, 222, 184),
    stars=0.0,
    milky=0.0,
    clouds=[((206, 212, 230), (250, 250, 255), (255, 255, 255), 0.6), ((212, 208, 224), (255, 246, 238), (255, 250, 240), 0.82)],
    cloud_glow=(255, 232, 204),
    rock=(104, 112, 138),
    forest=(52, 72, 78),
    snow=(250, 250, 255),
    ambient=((196, 210, 240), 1.25),
    light=((255, 226, 190), 0.75),
    haze=(0.9, 0.7, 0.96, (206, 216, 238), 0.6, 0.8, 0.3),
    water=((150, 172, 200), (0.97, 0.98, 1.0)),
    base=(233, 237, 244),
    css=dict(mist=(255, 255, 255, 0.8), ripple=(255, 255, 255, 0.7), halo=(255, 244, 222, 0.5), glint=(255, 250, 236, 0.85)),
    sun=0.4,
)

DAY = look(
    stops=[(58, 118, 214), (84, 144, 224), (120, 170, 232), (156, 194, 238), (188, 212, 242), (212, 226, 244), (226, 234, 244)],
    far=(222, 230, 242),
    near=(236, 240, 246),
    stars=0.0,
    milky=0.0,
    clouds=[((196, 206, 224), (255, 255, 255), (255, 255, 255), 0.62), ((200, 208, 224), (255, 255, 255), (255, 255, 255), 0.86)],
    cloud_glow=(255, 255, 250),
    rock=(118, 126, 140),
    forest=(58, 88, 74),
    snow=(250, 252, 255),
    ambient=((200, 214, 240), 1.25),
    light=((255, 250, 236), 1.0),
    haze=(0.84, 0.75, 1.0, (196, 212, 236), 0.6, 0.35, 0.22),
    water=((70, 120, 180), (0.95, 0.97, 1.0)),
    base=(214, 224, 240),
    css=dict(mist=(255, 255, 255, 0.5), ripple=(255, 255, 255, 0.65), halo=(255, 250, 230, 0.55), glint=(255, 255, 250, 0.9)),
    sun=0.3,
)

EVENING = [(-0.5, NIGHT), (-0.2, DUSK), (0.03, SUNSET), (0.25, GOLDEN), (0.6, DAY)]
MORNINGS = [(-0.5, NIGHT), (-0.2, DAWN), (0.03, SUNRISE), (0.25, MORNING), (0.6, DAY)]

def blend(a, b, t):
    if isinstance(a, dict):
        return {key: blend(a[key], b[key], t) for key in a}
    if isinstance(a, list):
        return [blend(x, y, t) for x, y in zip(a, b)]
    if isinstance(a, tuple):
        return a
    return a + (b - a) * t

def lake_look(hour):
    height = elevation(hour)
    track = MORNINGS if hour < 12.0 else EVENING
    if height <= track[0][0]:
        return track[0][1]
    for (low, a), (high, b) in zip(track, track[1:]):
        if height <= high:
            return blend(a, b, (height - low) / (high - low))
    return track[-1][1]

def key_id(hour):
    return f"{int(hour):02d}{int(round((hour % 1) * 60)):02d}"

def css_color(value):
    r, g, b, a = (float(v) for v in value)
    return [round(r), round(g), round(b), round(a, 3)]

def render_lake(scale, preview):
    scene = Scene("lake", scale, 2027)
    w, h, hy = scene.w, scene.h, scene.hy
    sky_rows = int(round(h * SKY_ROWS))
    land_top = int(round(h * LAND_TOP))
    OUT.mkdir(parents=True, exist_ok=True)
    for old in OUT.glob("lake-*"):
        old.unlink()
    keys = []
    for hour in KEY_HOURS:
        style = lake_look(hour)
        sun = body_position(SUN, hour)
        moon = body_position(MOON, hour)
        morning = hour < 12.0
        if sun:
            glow_x = sun[0]
        elif elevation(hour) > -0.5:
            glow_x = SUN["x0"] if morning else SUN["x1"]
        else:
            glow_x = moon[0] if moon else 0.5

        stops = list(zip(style["positions"], style["stops"]))
        sky, glow = scene.sky_gradient(stops, style["horizon_far"], style["horizon_near"], glow_x, style["glow_width"])
        if style["milky"] > 0.02:
            sky, density = scene.milky_way(sky, (0.02, 0.62), (0.7, -0.22), 0.1, 0.75 * style["milky"])
            sky = scene.stars(sky, 5200, 0.9 * style["stars"], band=density, stream=6)
        if style["stars"] > 0.02:
            sky = scene.stars(sky, 5200, style["stars"])
        if sun and style["sun"] > 0:
            sky = scene.sun_glow(sky, sun[0], sun[1], style["cloud_glow"], style["sun"])
        dither = scene.stream(7).uniform(-0.5, 0.5, sky.shape).astype(np.float32) / 255.0
        sky = np.clip(np.clip(sky, 0.0, 1.0) ** (1 / 2.2) + dither, 0.0, 1.0) ** 2.2

        layers = [
            scene.clouds(glow=glow, glow_tint=style["cloud_glow"], **shape, **colors)
            for shape, colors in zip(CLOUD_SHAPES, style["clouds"])
        ]

        lit_by = sun if sun and elevation(hour) > -0.02 else moon
        direction = ((lit_by[0] - 0.5) * 1.7, 0.24 + 0.5 * lit_by[2], 0.42) if lit_by else (0.0, 1.0, 0.2)
        light = dict(
            dir=direction,
            color=style["light"] * (1.0 if lit_by else 0.15),
            haze=style["haze"],
            haze_curve=style["haze_curve"],
            haze_gain=style["haze_gain"],
            veil=style["veil"],
            veil_mix=style["veil_mix"],
            mist=style["mist"],
            mist_height=style["mist_height"],
        )
        palette = dict(rock=style["rock"], forest=style["forest"], snow=style["snow"], ambient=style["ambient"])
        haze_row = sky[hy - max(2, int(6 * scene.unit))]
        color, cover = scene.land(RANGES, palette, haze_row, light)
        land_rows = color.shape[0]

        above = sky[:hy].copy()
        for layer in layers:
            rows = min(layer.shape[0], hy)
            above[:rows] = over(above[:rows], layer[:rows] * np.array([1, 1, 1, 0.75], dtype=np.float32))
        top = hy - land_rows
        above[top:hy] = above[top:hy] * (1.0 - cover[:, :, None]) + color
        water = scene.lake(above, style["deep"], style["tint"])

        rows_below = h - hy
        depth = (np.arange(rows_below, dtype=np.float32) / rows_below)[:, None, None]
        shore = np.exp(-np.arange(rows_below, dtype=np.float32) / (5.0 * scene.unit))[:, None, None]
        water = mix(water, np.broadcast_to(haze_row[None, :, :] * style["haze_gain"], water.shape), shore * 0.55)
        water = mix(water, style["base"][None, None, :], smoothstep(0.42, 0.9, depth))

        plate = np.zeros((h - land_top, w, 4), dtype=np.float32)
        start = top - land_top
        plate[start : start + land_rows, :, :3] = color / np.maximum(cover[:, :, None], 1e-4)
        plate[start : start + land_rows, :, 3] = cover
        plate[hy - land_top :, :, :3] = water
        plate[hy - land_top :, :, 3] = 1.0

        name = key_id(hour)
        save_rgb(OUT / f"lake-{name}-sky.webp", sky[:sky_rows], quality=88)
        save_rgba(OUT / f"lake-{name}-land.webp", plate, quality=86, width=round(w * 0.75))
        for index, layer in enumerate(layers):
            save_rgba(OUT / f"lake-{name}-clouds-{index}.webp", layer, quality=80, width=round(w * 0.5))

        header = encode(sky[: int(h * 0.26)]).astype(np.float32) / 255.0
        linear = np.where(header <= 0.03928, header / 12.92, ((header + 0.055) / 1.055) ** 2.4)
        keys.append(
            dict(
                hour=hour,
                id=name,
                base=[int(v) for v in encode(style["base"])],
                luminance=round(float(luminance(linear).mean()), 4),
                stars=round(float(style["stars"]), 3),
                **{key: css_color(value) for key, value in style["css"].items()},
            )
        )

        if preview or hour == 19.5:
            full = np.broadcast_to(style["base"][None, None, :], (h, w, 3)).copy()
            full[:sky_rows] = sky[:sky_rows]
            for layer in layers:
                full[: layer.shape[0]] = over(full[: layer.shape[0]], layer)
            full[land_top:] = over(full[land_top:], plate)
            if hour == 19.5:
                Image.fromarray(encode(full)).crop((int(w * 0.5), 0, w, int(h * 0.55))).resize((288, 180), Image.LANCZOS).save(
                    OUT / "lake-thumb.webp", "WEBP", quality=82, method=6
                )
            if preview:
                Image.fromarray(encode(full)).resize((1280, 720), Image.LANCZOS).save(pathlib.Path(preview) / f"lake-{name}.png")
        print(f"lake {name}: luminance {keys[-1]['luminance']}")

    meta = dict(
        horizon=HORIZON,
        skyRows=SKY_ROWS,
        landTop=LAND_TOP,
        cloudRows=CLOUD_ROWS,
        sun=SUN,
        moon=MOON,
        keys=keys,
    )
    (OUT / "lake.json").write_text(json.dumps(meta, indent=2) + "\n")

def render_moon():
    size = 256
    radius = 104.0
    rng = np.random.default_rng(31)
    yy, xx = np.mgrid[0:size, 0:size].astype(np.float32)
    nx, ny = (xx - size / 2) / radius, (yy - size / 2) / radius
    rr = np.hypot(nx, ny)
    maria = spectral(rng, size, size, 2.6, 2.6, 2.4, fmin=0.5)
    fine = spectral(rng, size, size, 2.6, 2.6, 1.4, fmin=3.0)
    seas = smoothstep(0.1, 0.9, maria + 0.35 * (0.2 - ny) - 0.25 * nx)
    shade = (1.0 - 0.3 * seas) * (1.0 + 0.045 * fine)
    limb = 0.8 + 0.2 * np.sqrt(np.clip(1.0 - rr * rr, 0.0, 1.0))
    disc = srgb(255, 250, 238)[None, None, :] * (shade * limb)[:, :, None] * 1.02
    cover = 1.0 - smoothstep(1.0 - 1.4 / radius, 1.0 + 1.4 / radius, rr)
    save_rgba(OUT / "moon.webp", np.concatenate([disc, cover[:, :, None]], axis=2), quality=90)

def render_hoshi(scale, preview):
    scene = Scene("hoshi", scale, 9091)
    w, h = scene.w, scene.h
    yy, xx = np.mgrid[0:h, 0:w].astype(np.float32)
    r = np.hypot((xx / w - 0.5) * 1.1, yy / h * 0.95)
    sky = mix(srgb(16, 20, 54)[None, None, :], srgb(4, 5, 14)[None, None, :], smoothstep(0.0, 1.0, r)[:, :, None]).astype(np.float32)
    veil = spectral(scene.rng, h, w, h / w, 1.0, 2.8, fmin=0.8)
    veil2 = spectral(scene.rng, h, w, h / w, 1.0, 2.8, fmin=0.8)
    sky = sky + smoothstep(0.2, 2.2, veil)[:, :, None] * srgb(110, 70, 170) * 0.1
    sky = sky + smoothstep(0.4, 2.4, veil2)[:, :, None] * srgb(170, 80, 110) * 0.07
    sky, density = scene.milky_way(sky, (-0.05, 0.92), (1.05, 0.12), 0.13, 1.9)
    sky = scene.stars(sky, 16000, 1.0, band=density, stream=6)
    sky = scene.stars(sky, 9000, 1.15)
    dither = scene.rng.uniform(-0.5, 0.5, sky.shape).astype(np.float32) / 255.0
    sky = np.clip(sky ** (1 / 2.2) + dither, 0.0, 1.0) ** 2.2
    OUT.mkdir(parents=True, exist_ok=True)
    save_rgb(OUT / "hoshi-sky.webp", sky, quality=88)
    Image.fromarray(encode(sky)).crop((int(w * 0.25), int(h * 0.2), int(w * 0.75), int(h * 0.75))).resize((288, 180), Image.LANCZOS).save(
        OUT / "hoshi-thumb.webp", "WEBP", quality=82, method=6
    )
    if preview:
        Image.fromarray(encode(sky)).save(pathlib.Path(preview) / "hoshi.png")

def render_strips(scale):
    rng = np.random.default_rng(77)
    w = int(BASE_W * scale * 0.5)
    rows = int(180 * scale)
    fog = spectral(rng, rows * 2, w, rows * 2 / w * 7.0, 1.0, 3.2, fmin=1.0)[:rows]
    t = (np.arange(rows, dtype=np.float32) / rows)[:, None]
    shape = np.exp(-(((t - 0.56) / 0.24) ** 2))
    alpha = smoothstep(-0.6, 2.0, fog) * shape
    white = np.ones((rows, w, 3), dtype=np.float32)
    save_rgba(OUT / "mist.webp", np.concatenate([white, alpha[:, :, None]], axis=2), quality=70)

    rows = int(420 * scale)
    lines = spectral(rng, rows * 4, w, 40.0, 6.0, 0.8, fmin=3.0)
    t = np.arange(rows, dtype=np.float32) / rows
    depth = 1.0 / (0.06 + t)
    source = (depth[0] - depth) / (depth[0] - depth[-1]) * (rows * 4 - 2)
    low = np.floor(source).astype(int)
    frac = (source - low)[:, None]
    field = lines[low] * (1.0 - frac) + lines[low + 1] * frac
    alpha = smoothstep(1.15, 2.4, field) * (smoothstep(0.0, 0.05, t) * (1.0 - smoothstep(0.5, 1.0, t)))[:, None]
    white = np.ones((rows, w, 3), dtype=np.float32)
    save_rgba(OUT / "ripple.webp", np.concatenate([white, alpha[:, :, None]], axis=2), quality=70)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--scale", type=float, default=1.0)
    parser.add_argument("--preview")
    parser.add_argument("--only")
    args = parser.parse_args()
    if args.preview:
        pathlib.Path(args.preview).mkdir(parents=True, exist_ok=True)
    wanted = args.only.split(",") if args.only else ["lake", "hoshi", "strips"]
    if "lake" in wanted:
        render_lake(args.scale, args.preview)
        render_moon()
    if "hoshi" in wanted:
        render_hoshi(args.scale, args.preview)
    if "strips" in wanted:
        render_strips(args.scale)
    print(f"wrote {OUT}")

if __name__ == "__main__":
    main()
