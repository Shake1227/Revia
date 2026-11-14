import clsx from "clsx";
import { AnimatePresence, motion } from "motion/react";
import { spring } from "../design/motion";
import type { ItemState, SyncState } from "../ipc/types";
import { Icons, type IconComponent } from "./icons";
import { Spinner } from "./Spinner";
import styles from "./StatusGlyph.module.css";

export type Tone = "synced" | "ahead" | "behind" | "diverged" | "dirty" | "conflict" | "error" | "special" | "neutral";

export const stateVisual: Record<SyncState, { tone: Tone; icon: IconComponent }> = {
  synced: { tone: "synced", icon: Icons.check },
  dirty: { tone: "dirty", icon: Icons.pencil },
  ahead: { tone: "ahead", icon: Icons.arrowUp },
  behind: { tone: "behind", icon: Icons.arrowDown },
  diverged: { tone: "diverged", icon: Icons.arrowDownUp },
  conflicted: { tone: "conflict", icon: Icons.warning },
  in_progress: { tone: "special", icon: Icons.merge },
  detached: { tone: "special", icon: Icons.unlink },
  no_remote: { tone: "neutral", icon: Icons.cloudOff },
  remote_not_found: { tone: "error", icon: Icons.circleX },
  auth_error: { tone: "error", icon: Icons.key },
  unreachable: { tone: "error", icon: Icons.wifiOff },
  unpublished: { tone: "neutral", icon: Icons.cloudUpload },
  not_initialized: { tone: "neutral", icon: Icons.circleDashed },
  missing: { tone: "error", icon: Icons.folderX },
  unknown: { tone: "neutral", icon: Icons.circleAlert },
};

export const itemStateVisual: Record<ItemState, { tone: Tone; icon: IconComponent }> = {
  queued: { tone: "neutral", icon: Icons.clock },
  running: { tone: "ahead", icon: Icons.refresh },
  succeeded: { tone: "synced", icon: Icons.check },
  skipped: { tone: "neutral", icon: Icons.minus },
  failed: { tone: "error", icon: Icons.x },
  needs_confirmation: { tone: "dirty", icon: Icons.circleAlert },
  conflict: { tone: "conflict", icon: Icons.warning },
  cancelled: { tone: "neutral", icon: Icons.ban },
};

interface StatusGlyphProps {
  state: SyncState | null;
  size?: "sm" | "md" | "lg";
  busy?: boolean;
  progress?: number | null;
  flash?: ItemState | null;
  className?: string;
}

const iconSize = { sm: 11, md: 13, lg: 17 };

export function StatusGlyph({ state, size = "md", busy, progress, flash, className }: StatusGlyphProps) {
  const visual = flash ? itemStateVisual[flash] : state ? stateVisual[state] : null;
  const Icon = visual?.icon;
  const key = busy ? "busy" : flash ? `flash-${flash}` : (state ?? "none");
  return (
    <span className={clsx(styles.glyph, styles[size], className)} data-tone={busy ? "ahead" : (visual?.tone ?? "neutral")} data-busy={busy || undefined}>
      <AnimatePresence mode="popLayout" initial={false}>
        <motion.span
          key={key}
          className={styles.inner}
          initial={{ opacity: 0, scale: 0.5 }}
          animate={{ opacity: 1, scale: 1, transition: flash === "succeeded" ? spring.lively : spring.snappy }}
          exit={{ opacity: 0, scale: 0.6, transition: { duration: 0.1 } }}
        >
          {busy ? <Spinner size={iconSize[size] + 3} progress={progress} /> : Icon ? <Icon size={iconSize[size]} strokeWidth={2} /> : <span className={styles.placeholder} />}
        </motion.span>
      </AnimatePresence>
      {flash === "succeeded" && <span className={styles.ripple} />}
    </span>
  );
}

export function ToneDot({ tone, className }: { tone: Tone; className?: string }) {
  return <span className={clsx(styles.dot, className)} data-tone={tone} />;
}
