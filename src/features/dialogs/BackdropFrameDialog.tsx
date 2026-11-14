import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { Button } from "../../components/Button";
import { Dialog } from "../../components/Dialog";
import { ImageBackdrop } from "../../components/ImageBackdrop";
import { Segmented } from "../../components/Segmented";
import { useT } from "../../i18n";
import { type ImageFrame, useAppStore } from "../../stores/app";
import { useBackgroundStore } from "../../stores/background";
import styles from "./BackdropFrameDialog.module.css";

interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

type Handle = "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w";
type Fit = ImageFrame["fit"];

const HANDLES: Handle[] = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];
const WHOLE: Rect = { x: 0, y: 0, w: 1, h: 1 };
const SMALLEST = 0.06;
const CANVAS = { width: 500, height: 360 };
const PREVIEW_WIDTH = 240;
const EDGE = { width: 48, height: 30 };

const clamp = (value: number, low: number, high: number) => Math.min(high, Math.max(low, value));

const move = (rect: Rect, dx: number, dy: number): Rect => ({
  ...rect,
  x: clamp(rect.x + dx, 0, 1 - rect.w),
  y: clamp(rect.y + dy, 0, 1 - rect.h),
});

function resize(start: Rect, handle: Handle, dx: number, dy: number, lock: number | null): Rect {
  const west = handle.includes("w");
  const east = handle.includes("e");
  const north = handle.includes("n");
  const south = handle.includes("s");
  let left = start.x;
  let top = start.y;
  let right = start.x + start.w;
  let bottom = start.y + start.h;
  if (west) left = clamp(left + dx, 0, right - SMALLEST);
  if (east) right = clamp(right + dx, left + SMALLEST, 1);
  if (north) top = clamp(top + dy, 0, bottom - SMALLEST);
  if (south) bottom = clamp(bottom + dy, top + SMALLEST, 1);
  if (lock === null) return { x: left, y: top, w: right - left, h: bottom - top };

  let w = right - left;
  let h = bottom - top;
  if (west || east) h = w * lock;
  else w = h / lock;
  const anchorX = west ? start.x + start.w : east ? start.x : start.x + start.w / 2;
  const anchorY = north ? start.y + start.h : south ? start.y : start.y + start.h / 2;
  const roomX = west ? anchorX : east ? 1 - anchorX : 2 * Math.min(anchorX, 1 - anchorX);
  const roomY = north ? anchorY : south ? 1 - anchorY : 2 * Math.min(anchorY, 1 - anchorY);
  const shrink = Math.min(1, roomX / w, roomY / h);
  w *= shrink;
  h *= shrink;
  return {
    x: west ? anchorX - w : east ? anchorX : anchorX - w / 2,
    y: north ? anchorY - h : south ? anchorY : anchorY - h / 2,
    w,
    h,
  };
}

function conform(rect: Rect, lock: number): Rect {
  const w = Math.min(rect.w, rect.h / lock, 1, 1 / lock);
  const h = w * lock;
  return {
    x: clamp(rect.x + rect.w / 2 - w / 2, 0, 1 - w),
    y: clamp(rect.y + rect.h / 2 - h / 2, 0, 1 - h),
    w,
    h,
  };
}

const percent = (value: number) => `${(value * 100).toFixed(3)}%`;

export function BackdropFrameDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const m = useT();
  const t = m.settings.background.frame;
  const url = useBackgroundStore((state) => state.url);
  const [natural, setNatural] = useState<{ width: number; height: number } | null>(null);
  const [rect, setRect] = useState<Rect>(WHOLE);
  const [fit, setFit] = useState<Fit>("fill");
  const [edge, setEdge] = useState<string | null>(null);
  const stage = useRef<HTMLDivElement>(null);
  const picture = useRef<HTMLImageElement>(null);
  const drag = useRef<{ target: "move" | Handle; x: number; y: number; rect: Rect } | null>(null);

  const ratio = useMemo(() => (open ? window.innerWidth / Math.max(1, window.innerHeight) : 16 / 10), [open]);
  const lock = natural ? natural.width / (natural.height * ratio) : null;

  useEffect(() => {
    if (!open || !natural || lock === null) return;
    const saved = useAppStore.getState().ui.background.frame;
    const nextFit = saved?.fit ?? "fill";
    const base = saved ? { x: saved.x, y: saved.y, w: saved.w, h: saved.h } : WHOLE;
    setFit(nextFit);
    setRect(nextFit === "fill" ? conform(base, lock) : base);
  }, [open, natural, lock]);

  useEffect(() => {
    if (!open) setNatural(null);
  }, [open, url]);

  useEffect(() => {
    if (!open || fit !== "show" || !natural || !picture.current) {
      setEdge(null);
      return;
    }
    const image = picture.current;
    const frame = requestAnimationFrame(() => {
      try {
        const canvas = document.createElement("canvas");
        canvas.width = EDGE.width;
        canvas.height = EDGE.height;
        const context = canvas.getContext("2d");
        if (!context) return;
        context.drawImage(
          image,
          rect.x * natural.width,
          rect.y * natural.height,
          rect.w * natural.width,
          rect.h * natural.height,
          0,
          0,
          EDGE.width,
          EDGE.height,
        );
        setEdge(canvas.toDataURL("image/jpeg", 0.72));
      } catch {
        setEdge(null);
      }
    });
    return () => cancelAnimationFrame(frame);
  }, [open, fit, natural, rect]);

  const chooseFit = (next: Fit) => {
    setFit(next);
    if (next === "fill" && lock !== null) setRect((current) => conform(current, lock));
  };

  const begin = (target: "move" | Handle) => (event: PointerEvent<HTMLElement>) => {
    event.preventDefault();
    event.stopPropagation();
    try {
      event.currentTarget.setPointerCapture(event.pointerId);
    } catch {
    }
    drag.current = { target, x: event.clientX, y: event.clientY, rect };
  };

  const track = (event: PointerEvent<HTMLElement>) => {
    const current = drag.current;
    const box = stage.current?.getBoundingClientRect();
    if (!current || !box) return;
    const dx = (event.clientX - current.x) / box.width;
    const dy = (event.clientY - current.y) / box.height;
    setRect(current.target === "move" ? move(current.rect, dx, dy) : resize(current.rect, current.target, dx, dy, fit === "fill" ? lock : null));
  };

  const finish = () => {
    drag.current = null;
  };

  const nudge = (event: KeyboardEvent<HTMLDivElement>) => {
    const step = event.shiftKey ? 0.05 : 0.01;
    const by: Record<string, [number, number]> = {
      ArrowLeft: [-step, 0],
      ArrowRight: [step, 0],
      ArrowUp: [0, -step],
      ArrowDown: [0, step],
    };
    const delta = by[event.key];
    if (!delta) return;
    event.preventDefault();
    setRect((current) => move(current, delta[0], delta[1]));
  };

  const apply = () => {
    if (!natural) return;
    const { ui, setUi } = useAppStore.getState();
    const frame: ImageFrame = {
      ...rect,
      aspect: (rect.w * natural.width) / (rect.h * natural.height),
      fit,
      edge: fit === "show" ? edge : null,
    };
    setUi({ background: { ...ui.background, frame } });
    onClose();
  };

  const reset = () => {
    const { ui, setUi } = useAppStore.getState();
    setUi({ background: { ...ui.background, frame: null } });
    onClose();
  };

  const scale = natural ? Math.min(CANVAS.width / natural.width, CANVAS.height / natural.height) : 1;
  const preview = { width: PREVIEW_WIDTH, height: Math.round(PREVIEW_WIDTH / ratio) };
  const frame: ImageFrame | null = natural
    ? { ...rect, aspect: (rect.w * natural.width) / (rect.h * natural.height), fit, edge }
    : null;

  return (
    <Dialog
      open={open && url !== null}
      onOpenChange={(next) => !next && onClose()}
      title={t.title}
      description={t.description}
      size="xl"
      flush
      footerStart={
        <Button variant="ghost" size="sm" onClick={reset}>
          {t.reset}
        </Button>
      }
      footer={
        <>
          <Button onClick={onClose}>{m.common.cancel}</Button>
          <Button variant="primary" disabled={!natural} onClick={apply}>
            {t.apply}
          </Button>
        </>
      }
    >
      {url && (
        <div className={styles.editor}>
          <div className={styles.canvas}>
            <div
              ref={stage}
              className={styles.stage}
              style={natural ? { width: natural.width * scale, height: natural.height * scale } : { width: CANVAS.width, height: CANVAS.height }}
            >
              <img
                ref={picture}
                className={styles.source}
                src={url}
                alt=""
                draggable={false}
                onLoad={(event) => setNatural({ width: event.currentTarget.naturalWidth, height: event.currentTarget.naturalHeight })}
              />
              {natural && (
                <div
                  className={styles.crop}
                  style={{ left: percent(rect.x), top: percent(rect.y), width: percent(rect.w), height: percent(rect.h) }}
                  role="group"
                  aria-label={t.move}
                  tabIndex={0}
                  onPointerDown={begin("move")}
                  onPointerMove={track}
                  onPointerUp={finish}
                  onPointerCancel={finish}
                  onKeyDown={nudge}
                >
                  {HANDLES.map((handle) => (
                    <span
                      key={handle}
                      className={styles.handle}
                      data-handle={handle}
                      aria-hidden
                      onPointerDown={begin(handle)}
                      onPointerMove={track}
                      onPointerUp={finish}
                      onPointerCancel={finish}
                    />
                  ))}
                </div>
              )}
            </div>
          </div>

          <div className={styles.side}>
            <span className={styles.label}>{t.preview}</span>
            <div className={styles.preview} style={preview}>
              {frame && <ImageBackdrop url={url} frame={frame} blur={0} size={preview} />}
              <span className={styles.mockBar} />
              <span className={styles.mockSheet} />
            </div>

            <span className={styles.label}>{t.mode}</span>
            <Segmented<Fit>
              size="sm"
              value={fit}
              onChange={chooseFit}
              ariaLabel={t.mode}
              options={[
                { value: "fill", label: t.fill },
                { value: "show", label: t.show },
              ]}
            />
            <p className={styles.hint}>{fit === "fill" ? t.fillHint : t.showHint}</p>
          </div>
        </div>
      )}
    </Dialog>
  );
}
