import { type CSSProperties, useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { Tooltip } from "../../components/Tooltip";
import { useT } from "../../i18n";
import { windowFrame } from "../../ipc/window";
import { notifyError } from "../../lib/notify";
import styles from "./WindowControls.module.css";

type Control = "minimize" | "maximize" | "close";

const ORDER: Control[] = ["minimize", "maximize", "close"];

const ACTIONS: Record<Control, () => Promise<void>> = {
  minimize: () => windowFrame.minimize(),
  maximize: () => windowFrame.toggleMaximize(),
  close: () => windowFrame.close(),
};

interface Hover {
  slot: number;
  live: boolean;
  glide: boolean;
}

const REST: Hover = { slot: 0, live: false, glide: false };

function Glyph({ control, maximized }: { control: Control; maximized: boolean }) {
  return (
    <svg
      className={styles.glyph}
      viewBox="0 0 12 12"
      width="12"
      height="12"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.3"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      {control === "minimize" && <path d="M2.5 6.5h7" />}
      {control === "maximize" && !maximized && <rect x="2.5" y="2.5" width="7" height="7" rx="1.8" />}
      {control === "maximize" && maximized && (
        <>
          <rect x="2" y="4" width="6" height="6" rx="1.6" />
          <path d="M4.6 4v-.4A1.6 1.6 0 0 1 6.2 2h2.2A1.6 1.6 0 0 1 10 3.6v2.2a1.6 1.6 0 0 1-1.6 1.6H8" />
        </>
      )}
      {control === "close" && <path d="M3 3l6 6M9 3l-6 6" />}
    </svg>
  );
}

export function WindowControls() {
  const m = useT();
  const [maximized, setMaximized] = useState(false);
  const [focused, setFocused] = useState(() => document.hasFocus());
  const [hover, setHover] = useState<Hover>(REST);

  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    let timer: number | undefined;
    const read = () => {
      void windowFrame.isMaximized().then(
        (value) => {
          if (!disposed) setMaximized(value);
        },
        () => undefined,
      );
    };
    read();
    void windowFrame
      .onResized(() => {
        window.clearTimeout(timer);
        timer = window.setTimeout(read, 80);
      })
      .then(
        (unlisten) => {
          if (disposed) unlisten();
          else stop = unlisten;
        },
        () => undefined,
      );
    return () => {
      disposed = true;
      window.clearTimeout(timer);
      stop?.();
    };
  }, []);

  useEffect(() => {
    const onFocus = () => setFocused(true);
    const onBlur = () => {
      setFocused(false);
      setHover(REST);
    };
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    return () => {
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
    };
  }, []);

  const labels: Record<Control, string> = {
    minimize: m.window.minimize,
    maximize: maximized ? m.window.restore : m.window.maximize,
    close: m.window.close,
  };
  const current = hover.live ? ORDER[hover.slot] : undefined;

  return createPortal(
    <div
      className={styles.controls}
      data-surface
      data-inactive={!focused || undefined}
      data-live={hover.live || undefined}
      data-glide={hover.glide || undefined}
      data-danger={current === "close" || undefined}
      style={{ "--slot": hover.slot } as CSSProperties}
      onPointerDown={(event) => event.stopPropagation()}
      onPointerLeave={() => setHover((state) => ({ ...state, live: false, glide: false }))}
    >
      <span className={styles.pebble} aria-hidden />
      {ORDER.map((control, slot) => (
        <Tooltip key={control} label={labels[control]} align={control === "close" ? "end" : "center"}>
          <button
            type="button"
            className={styles.button}
            data-control={control}
            data-current={current === control || undefined}
            tabIndex={-1}
            aria-label={labels[control]}
            onMouseDown={(event) => event.preventDefault()}
            onPointerMove={() =>
              setHover((state) => (state.live && state.slot === slot ? state : { slot, live: true, glide: state.live }))
            }
            onClick={() => {
              setHover((state) => ({ ...state, live: false, glide: false }));
              void ACTIONS[control]().catch(notifyError);
            }}
          >
            <Glyph control={control} maximized={maximized} />
          </button>
        </Tooltip>
      ))}
    </div>,
    document.body,
  );
}
