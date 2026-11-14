import meta from "../assets/backdrop/lake.json";

type Rgba = [number, number, number, number];

interface RawKey {
  hour: number;
  id: string;
  base: number[];
  luminance: number;
  stars: number;
  mist: number[];
  ripple: number[];
  halo: number[];
  glint: number[];
}

interface Arc {
  rise: number;
  set: number;
  x0: number;
  x1: number;
  peak: number;
  shape: number;
}

export interface LakeKey {
  hour: number;
  id: string;
  sky: string;
  land: string;
  clouds: [string, string];
}

export interface Body {
  x: number;
  y: number;
  lift: number;
}

export interface LakeView {
  from: LakeKey;
  to: LakeKey;
  mix: number;
  base: string;
  mist: string;
  ripple: string;
  halo: string;
  glint: string;
  stars: number;
  luminance: number;
  sun: Body | null;
  moon: Body | null;
}

const urls = import.meta.glob<string>("../assets/backdrop/lake-*.webp", { eager: true, query: "?url", import: "default" });
const url = (name: string) => urls[`../assets/backdrop/${name}.webp`] ?? "";

const raw = [...(meta.keys as RawKey[])].sort((a, b) => a.hour - b.hour);

export const LAKE_KEYS: LakeKey[] = raw.map((key) => ({
  hour: key.hour,
  id: key.id,
  sky: url(`lake-${key.id}-sky`),
  land: url(`lake-${key.id}-land`),
  clouds: [url(`lake-${key.id}-clouds-0`), url(`lake-${key.id}-clouds-1`)],
}));

export const LAKE_IMAGES: string[] = LAKE_KEYS.flatMap((key) => [key.sky, key.land, ...key.clouds]);

const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
const css = (a: number[], b: number[], t: number) => {
  const [r, g, bl, alpha] = [0, 1, 2, 3].map((index) => lerp(a[index] ?? 1, b[index] ?? 1, t)) as Rgba;
  return `rgb(${Math.round(r)} ${Math.round(g)} ${Math.round(bl)} / ${alpha.toFixed(3)})`;
};

function body(arc: Arc, hour: number): Body | null {
  const h = hour >= arc.rise ? hour : hour + 24;
  if (h < arc.rise || h > arc.set) return null;
  const progress = (h - arc.rise) / (arc.set - arc.rise);
  const lift = Math.sin(Math.PI * progress);
  return { x: arc.x0 + (arc.x1 - arc.x0) * progress, y: meta.horizon - lift ** arc.shape * arc.peak, lift };
}

export const wrapHour = (hour: number) => ((hour % 24) + 24) % 24;

export function lakeAt(time: number): LakeView {
  const hour = wrapHour(time);
  let index = raw.length - 1;
  for (let i = 0; i < raw.length; i++) {
    if (raw[i]!.hour <= hour) index = i;
  }
  const next = (index + 1) % raw.length;
  const a = raw[index]!;
  const b = raw[next]!;
  const span = next === 0 ? 24 - a.hour + b.hour : b.hour - a.hour;
  const mix = Math.min(1, Math.max(0, (hour - a.hour) / span));
  return {
    from: LAKE_KEYS[index]!,
    to: LAKE_KEYS[next]!,
    mix,
    base: css([...a.base, 1], [...b.base, 1], mix),
    mist: css(a.mist, b.mist, mix),
    ripple: css(a.ripple, b.ripple, mix),
    halo: css(a.halo, b.halo, mix),
    glint: css(a.glint, b.glint, mix),
    stars: lerp(a.stars, b.stars, mix),
    luminance: lerp(a.luminance, b.luminance, mix),
    sun: body(meta.sun, hour),
    moon: body(meta.moon, hour),
  };
}

export const skyTone = (hour: number): "light" | "dark" => (lakeAt(hour).luminance > 0.2 ? "light" : "dark");

export function clockLabel(hour: number): string {
  const total = Math.round(wrapHour(hour) * 60) % (24 * 60);
  return `${String(Math.floor(total / 60)).padStart(2, "0")}:${String(total % 60).padStart(2, "0")}`;
}

export function currentHour(now = new Date()): number {
  return now.getHours() + now.getMinutes() / 60 + now.getSeconds() / 3600;
}
