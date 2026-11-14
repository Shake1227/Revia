import { motion } from "motion/react";
import { CountUp } from "../../components/CountUp";
import type { Tone } from "../../components/StatusGlyph";
import { Tooltip } from "../../components/Tooltip";
import { spring } from "../../design/motion";
import { useT } from "../../i18n";
import { useAppStore, type SummaryFilter } from "../../stores/app";
import { SUMMARY_FILTERS } from "./model";
import styles from "./SummaryStrip.module.css";

const tones: Record<SummaryFilter, Tone | null> = {
  all: null,
  synced: "synced",
  dirty: "dirty",
  ahead: "ahead",
  behind: "behind",
  unpublished: "neutral",
  attention: "conflict",
};

export function SummaryStrip({ counts }: { counts: Record<SummaryFilter, number> }) {
  const m = useT();
  const filter = useAppStore((state) => state.ui.filter);
  const setUi = useAppStore((state) => state.setUi);

  return (
    <div className={styles.summary}>
    <div className={styles.strip} role="tablist" aria-label={m.projects.summary.all}>
      {SUMMARY_FILTERS.map((key) => {
        const active = filter === key;
        const tone = tones[key];
        const value = counts[key];
        return (
          <Tooltip key={key} label={m.projects.summaryHint[key]} side="bottom">
            <button
              type="button"
              role="tab"
              aria-selected={active}
              className={styles.cell}
              data-active={active || undefined}
              data-tone={tone ?? undefined}
              data-zero={value === 0 || undefined}
              onClick={() => setUi({ filter: active && key !== "all" ? "all" : key })}
            >
              <span className={styles.value}>
                <CountUp value={value} />
              </span>
              <span className={styles.label}>
                {m.projects.summary[key]}
              </span>
              {active && <motion.span layoutId="summary-active" className={styles.indicator} transition={spring.snappy} />}
            </button>
          </Tooltip>
        );
      })}
    </div>
    </div>
  );
}
