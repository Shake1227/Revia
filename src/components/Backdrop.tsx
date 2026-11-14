import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import hoshiSky from "../assets/backdrop/hoshi-sky.webp";
import mist from "../assets/backdrop/mist.webp";
import moonImage from "../assets/backdrop/moon.webp";
import ripple from "../assets/backdrop/ripple.webp";
import type { ImageFrame } from "../stores/app";
import styles from "./Backdrop.module.css";
import { ImageBackdrop } from "./ImageBackdrop";
import { LAKE_IMAGES, lakeAt, wrapHour, type Body } from "./lake";

export type Scene = "lake" | "hoshi" | "plain" | "image";

interface BackdropProps {
  scene: Scene;
  hour: number;
  live: boolean;
  imageUrl: string | null;
  frame: ImageFrame | null;
  dim: number;
  blur: number;
  animated: boolean;
  celebrate: number;
}

const url = (source: string) => `url("${source}")`;

function seeded(seed: number) {
  let value = seed;
  return () => {
    value = (value * 1664525 + 1013904223) % 4294967296;
    return value / 4294967296;
  };
}

function starField(seed: number, count: number, size: number): string {
  const random = seeded(seed);
  return Array.from({ length: count }, () => {
    const x = (random() * 100).toFixed(2);
    const y = (random() * 100).toFixed(2);
    const alpha = (0.4 + random() * 0.6).toFixed(2);
    const radius = (size * (0.6 + random() * 0.8)).toFixed(2);
    return `radial-gradient(${radius}px ${radius}px at ${x}% ${y}%, rgb(255 255 255 / ${alpha}), transparent 100%)`;
  }).join(",");
}

function motes(seed: number, count: number): CSSProperties[] {
  const random = seeded(seed);
  return Array.from({ length: count }, () => {
    const size = 1.6 + random() * 2;
    return {
      left: `${(4 + random() * 92).toFixed(2)}%`,
      top: `calc(var(--horizon) + ${(-60 + random() * 140).toFixed(0)}px)`,
      width: size,
      height: size,
      animationDuration: `${(9 + random() * 12).toFixed(1)}s, ${(2.4 + random() * 3).toFixed(1)}s`,
      animationDelay: `${(-random() * 20).toFixed(1)}s, ${(-random() * 4).toFixed(1)}s`,
      "--sway": `${(random() > 0.5 ? 1 : -1) * (14 + random() * 34)}px`,
      "--rise": `${-(30 + random() * 70)}px`,
    } as CSSProperties;
  });
}

function glints(seed: number, count: number): CSSProperties[] {
  const random = seeded(seed);
  return Array.from({ length: count }, (_, index) => ({
    top: `${(4 + index * (92 / count) + random() * 3).toFixed(1)}%`,
    width: `${(18 + index * 3.4 + random() * 34).toFixed(0)}px`,
    marginLeft: `${((random() - 0.5) * (14 + index * 2.4)).toFixed(0)}px`,
    animationDuration: `${(2.4 + random() * 3.2).toFixed(1)}s`,
    animationDelay: `${(-random() * 5).toFixed(1)}s`,
  }));
}

function streaks(seed: number, count: number): CSSProperties[] {
  const random = seeded(seed);
  return Array.from({ length: count }, () => ({
    top: `${(4 + random() * 78).toFixed(1)}%`,
    width: 18 + random() * 34,
    height: 1 + random() * 0.6,
    animationDuration: `${(11 + random() * 12).toFixed(1)}s`,
    animationDelay: `${(-random() * 24).toFixed(1)}s`,
    "--drop": `${((random() - 0.3) * 46).toFixed(0)}px`,
    "--turn": `${((random() - 0.5) * 10).toFixed(0)}deg`,
  })) as CSSProperties[];
}

interface Petal {
  flight: CSSProperties;
  sway: CSSProperties;
  tumble: CSSProperties;
}

function petals(seed: number, count: number): Petal[] {
  const random = seeded(seed);
  return Array.from({ length: count }, () => {
    const depth = random();
    const size = 8 + depth * 9;
    return {
      flight: {
        top: `${(3 + random() * 58).toFixed(1)}%`,
        width: size,
        height: size,
        animationDuration: `${(14 - depth * 5 + random() * 3).toFixed(1)}s`,
        animationDelay: `${(-random() * 24).toFixed(1)}s`,
        "--drop": `${(30 + random() * 150).toFixed(0)}px`,
        "--near": (0.5 + depth * 0.5).toFixed(2),
      } as CSSProperties,
      sway: {
        animationDuration: `${(2.6 + random() * 2.4).toFixed(1)}s`,
        animationDelay: `${(-random() * 5).toFixed(1)}s`,
        "--sway": `${(7 + random() * 15).toFixed(0)}px`,
      } as CSSProperties,
      tumble: {
        animationDuration: `${(3 + random() * 3.4).toFixed(1)}s`,
        animationDelay: `${(-random() * 6).toFixed(1)}s`,
        "--lean": `${(random() * 360).toFixed(0)}deg`,
      } as CSSProperties,
    };
  });
}

const PETAL_SHAPE =
  "M10 19.200C7.200 16 4.600 12 5 7.600 5.200 4.600 6.600 2.400 8.300 2c.800-.100 1.400.500 1.700 1.600.300-1.100.900-1.700 1.700-1.600 1.700.400 3.100 2.600 3.300 5.600.400 4.400-2.200 8.400-5 11.600Z";

const place = (body: Body) => ({ "--bx": body.x.toFixed(5), "--by": body.y.toFixed(5) }) as CSSProperties;

function useGlidingHour(target: number, animated: boolean): [number, boolean] {
  const [shown, setShown] = useState(target);
  const [gliding, setGliding] = useState(false);
  const current = useRef(target);

  useEffect(() => {
    const from = current.current;
    const delta = ((target - from + 36) % 24) - 12;
    if (!animated || Math.abs(delta) < 2 / 60) {
      current.current = target;
      setShown(target);
      return;
    }
    const duration = Math.min(2800, 700 + Math.abs(delta) * 240);
    const start = performance.now();
    let frame = 0;
    setGliding(true);
    const step = (now: number) => {
      const t = Math.min(1, (now - start) / duration);
      const eased = t < 0.5 ? 4 * t ** 3 : 1 - (-2 * t + 2) ** 3 / 2;
      current.current = wrapHour(from + delta * eased);
      setShown(current.current);
      if (t < 1) frame = requestAnimationFrame(step);
      else setGliding(false);
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  }, [target, animated]);

  return [shown, gliding];
}

function Sun({ body }: { body: Body }) {
  const height = Math.min(1, body.lift / 0.5);
  const color = `rgb(255 ${Math.round(150 + 96 * height)} ${Math.round(84 + 136 * height)})`;
  const style = { ...place(body), "--sun-color": color, "--sun-scale": (1.4 - 0.4 * height).toFixed(3), "--sun-rays": (0.14 + 0.3 * height).toFixed(3) } as CSSProperties;
  return (
    <div className={styles.sun} style={style}>
      <div className={styles.sunGlow} />
      <div className={styles.sunRays} />
      <div className={styles.sunDisc} />
    </div>
  );
}

function Moon({ body, strength }: { body: Body; strength: number }) {
  return (
    <div className={styles.moon} style={{ ...place(body), opacity: 0.6 + 0.4 * strength }}>
      <div className={styles.moonHalo} />
      <img className={styles.moonDisc} src={moonImage} alt="" />
    </div>
  );
}

function useWind(root: React.RefObject<HTMLDivElement | null>, enabled: boolean) {
  const [gust, setGust] = useState(0);

  useEffect(() => {
    const element = root.current;
    if (!element || !enabled) return;
    let rate = 1;
    let target = 1;
    let calmAt = 0;
    let nextGustAt = performance.now() + 6000 + Math.random() * 9000;

    const tick = window.setInterval(() => {
      const now = performance.now();
      if (target > 1 && now >= calmAt) target = 1;
      if (target === 1 && now >= nextGustAt) {
        target = 2.1 + Math.random() * 1.3;
        calmAt = now + 5000 + Math.random() * 4500;
        nextGustAt = calmAt + 14000 + Math.random() * 26000;
        setGust((count) => count + 1);
      }
      const next = rate + (target - rate) * (target > rate ? 0.16 : 0.07);
      if (Math.abs(next - rate) < 0.004) return;
      rate = next;
      element.style.setProperty("--wind", ((rate - 1) / 2.4).toFixed(3));
      for (const animation of element.getAnimations({ subtree: true })) {
        const owner = (animation.effect as KeyframeEffect | null)?.target;
        if (owner instanceof HTMLElement && owner.dataset.wind !== undefined) animation.playbackRate = rate;
      }
    }, 240);

    return () => {
      window.clearInterval(tick);
      element.style.setProperty("--wind", "0");
    };
  }, [root, enabled]);

  return gust;
}

export function Backdrop({ scene, hour, live, imageUrl, frame, dim, blur, animated, celebrate }: BackdropProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [active, setActive] = useState(true);
  const stars = useMemo(
    () => (scene === "hoshi" ? [starField(11, 70, 1), starField(29, 40, 1.4), starField(47, 16, 2)] : [starField(11, 36, 1), starField(29, 18, 1.3), starField(47, 7, 1.8)]),
    [scene],
  );
  const fireflies = useMemo(() => motes(5, 18), []);
  const reflections = useMemo(() => glints(13, 18), []);
  const windStreaks = useMemo(() => streaks(71, 12), []);
  const windPetals = useMemo(() => petals(97, 10), []);

  useEffect(() => {
    const update = () => setActive(document.visibilityState === "visible" && document.hasFocus());
    update();
    window.addEventListener("focus", update);
    window.addEventListener("blur", update);
    document.addEventListener("visibilitychange", update);
    return () => {
      window.removeEventListener("focus", update);
      window.removeEventListener("blur", update);
      document.removeEventListener("visibilitychange", update);
    };
  }, []);

  const moving = animated && active;
  const [shownHour, gliding] = useGlidingHour(hour, animated);
  const lake = scene === "lake" ? lakeAt(shownHour) : null;
  const gust = useWind(ref, moving && lake !== null);

  useEffect(() => {
    if (scene !== "lake") return;
    const timer = window.setTimeout(() => {
      for (const source of LAKE_IMAGES) new Image().src = source;
    }, 1500);
    return () => window.clearTimeout(timer);
  }, [scene]);

  const night = scene === "hoshi" || (lake !== null && lake.stars > 0.05);
  const vars = {
    "--mist": url(mist),
    "--ripple": url(ripple),
    ...(lake
      ? {
          "--base-color": lake.base,
          "--mist-color": lake.mist,
          "--ripple-color": lake.ripple,
          "--halo-color": lake.halo,
          "--glint-color": lake.glint,
        }
      : {}),
  } as CSSProperties;

  return (
    <div ref={ref} className={styles.backdrop} data-scene={scene} data-animated={moving} data-live={(live && !gliding) || undefined} style={vars} aria-hidden>
      {scene === "image" && imageUrl && (
        <>
          <ImageBackdrop url={imageUrl} frame={frame} blur={blur} />
          <div className={styles.dim} style={{ opacity: dim }} />
        </>
      )}

      {scene === "hoshi" && (
        <div className={styles.cosmos}>
          <div className={styles.cosmosPlate} style={{ backgroundImage: url(hoshiSky) }} />
          {stars.map((backgroundImage, index) => (
            <div key={index} className={styles.stars} data-layer={index} style={{ backgroundImage }} />
          ))}
        </div>
      )}

      {lake && (
        <div className={styles.plates}>
          <div className={styles.sky} style={{ backgroundImage: url(lake.from.sky) }} />
          <div className={styles.sky} style={{ backgroundImage: url(lake.to.sky), opacity: lake.mix }} />
          {lake.stars > 0.05 && (
            <div className={styles.starBand} style={{ opacity: lake.stars }}>
              {stars.map((backgroundImage, index) => (
                <div key={index} className={styles.stars} data-layer={index} style={{ backgroundImage }} />
              ))}
            </div>
          )}
          {lake.moon && <Moon body={lake.moon} strength={Math.min(1, lake.stars * 1.6)} />}
          {lake.sun && <Sun body={lake.sun} />}
          {[0, 1].map((layer) => (
            <div key={layer} className={styles.clouds} data-layer={layer}>
              <div className={styles.cloudTile} data-wind>
                <div className={styles.cloudImage} style={{ backgroundImage: url(lake.from.clouds[layer as 0 | 1]) }} />
                <div className={styles.cloudImage} style={{ backgroundImage: url(lake.to.clouds[layer as 0 | 1]), opacity: lake.mix }} />
              </div>
            </div>
          ))}
          <div className={styles.land} style={{ backgroundImage: url(lake.from.land) }} />
          <div className={styles.land} style={{ backgroundImage: url(lake.to.land), opacity: lake.mix }} />
          <div className={styles.water}>
            <div className={styles.ripples} data-layer="0" data-wind />
            <div className={styles.ripples} data-layer="1" data-wind />
            {lake.sun && (
              <>
                <div className={styles.column} data-body="sun" style={{ ...place(lake.sun), opacity: 0.2 + 0.4 * (1 - lake.sun.lift) }} />
                <div className={styles.reflection} data-body="sun" style={place(lake.sun)}>
                  {reflections.map((style, index) => (
                    <span key={index} className={styles.glint} style={style} data-wind />
                  ))}
                </div>
              </>
            )}
            {lake.moon && !lake.sun && (
              <>
                <div className={styles.column} data-body="moon" style={{ ...place(lake.moon), opacity: (0.12 + 0.2 * (1 - lake.moon.lift)) * Math.min(1, lake.stars * 1.6) }} />
                <div className={styles.reflection} data-body="moon" style={{ ...place(lake.moon), opacity: Math.min(1, lake.stars * 1.6) }}>
                  {reflections.slice(0, 11).map((style, index) => (
                    <span key={index} className={styles.glint} style={style} data-wind />
                  ))}
                </div>
              </>
            )}
            {gust > 0 && <div key={gust} className={styles.sweep} />}
          </div>
          <div className={styles.mist} data-layer="0">
            <div className={styles.mistTile} data-wind />
          </div>
          <div className={styles.mist} data-layer="1">
            <div className={styles.mistTile} data-wind />
          </div>
          {lake.stars > 0.5 && fireflies.map((style, index) => <span key={index} className={styles.mote} style={style} />)}
          <div className={styles.air}>
            <svg className={styles.petalInk} aria-hidden>
              <defs>
                <linearGradient id="rv-petal" x1="0.5" y1="0" x2="0.5" y2="1">
                  <stop offset="0" className={styles.petalTip} />
                  <stop offset="0.55" className={styles.petalBody} />
                  <stop offset="1" className={styles.petalBase} />
                </linearGradient>
              </defs>
            </svg>
            {windStreaks.map((style, index) => (
              <span key={index} className={styles.streak} style={style} data-wind />
            ))}
            {windPetals.map((petal, index) => (
              <span key={index} className={styles.petal} style={petal.flight} data-wind>
                <span className={styles.petalSway} style={petal.sway}>
                  <svg className={styles.petalShape} style={petal.tumble} viewBox="0 0 20 20" aria-hidden>
                    <path d={PETAL_SHAPE} />
                  </svg>
                </span>
              </span>
            ))}
          </div>
        </div>
      )}

      {night && (
        <>
          <div className={styles.meteor} data-ambient />
          {celebrate > 0 && <div key={celebrate} className={styles.meteor} data-once />}
        </>
      )}
      <div className={styles.vignette} data-bright={lake !== null && lake.luminance > 0.2 ? "" : undefined} />
      <div className={styles.noise} />
    </div>
  );
}
