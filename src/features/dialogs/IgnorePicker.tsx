import { useEffect, useState } from "react";
import { Checkbox } from "../../components/Checkbox";
import { ErrorNote } from "../../components/ErrorNote";
import { Spinner } from "../../components/Spinner";
import { Switch } from "../../components/Switch";
import { useT } from "../../i18n";
import { api } from "../../ipc/client";
import { toAppError } from "../../ipc/errors";
import type { AppError, IgnoreCategory, IgnoreScan, IgnoreSuggestion, Project } from "../../ipc/types";
import styles from "./IgnorePicker.module.css";

const CATEGORY_ORDER: IgnoreCategory[] = ["secret", "personal", "system", "dependency", "build", "scratch"];

type ScanState = { status: "loading" } | { status: "ready"; scan: IgnoreScan } | { status: "failed"; error: AppError };

export type IgnoreSelection = Record<string, string[]>;

export function useIgnoreSelection(projects: Project[], enabled: boolean) {
  const [scans, setScans] = useState<Record<string, ScanState>>({});
  const [selection, setSelection] = useState<IgnoreSelection>({});
  const key = enabled ? projects.map((project) => project.id).join(",") : "";

  useEffect(() => {
    setScans({});
    setSelection({});
    if (key === "") return;
    let stale = false;
    for (const projectId of key.split(",")) {
      setScans((current) => ({ ...current, [projectId]: { status: "loading" } }));
      api
        .scanIgnoreCandidates({ projectId })
        .then((scan) => {
          if (stale) return;
          setScans((current) => ({ ...current, [projectId]: { status: "ready", scan } }));
          setSelection((current) => ({ ...current, [projectId]: scan.suggestions.map((item) => item.pattern) }));
        })
        .catch((reason: unknown) => {
          if (!stale) setScans((current) => ({ ...current, [projectId]: { status: "failed", error: toAppError(reason) } }));
        });
    }
    return () => {
      stale = true;
    };
  }, [key]);

  const toggle = (projectId: string, patterns: string[], checked: boolean) =>
    setSelection((current) => {
      const next = new Set(current[projectId] ?? []);
      for (const pattern of patterns) {
        if (checked) next.add(pattern);
        else next.delete(pattern);
      }
      return { ...current, [projectId]: [...next] };
    });

  const loading = Object.values(scans).some((state) => state.status === "loading");
  return { scans, selection, toggle, loading };
}

function Suggestions({
  scan,
  selected,
  onToggle,
}: {
  scan: IgnoreScan;
  selected: Set<string>;
  onToggle: (patterns: string[], checked: boolean) => void;
}) {
  const m = useT();
  const t = m.github.publish.ignore;
  if (scan.suggestions.length === 0) return <p className={styles.note}>{t.none}</p>;

  const groups = CATEGORY_ORDER.map((category) => ({ category, items: scan.suggestions.filter((item) => item.category === category) })).filter(
    (group) => group.items.length > 0,
  );
  const describe = (item: IgnoreSuggestion) => {
    const shown = item.examples.filter((example) => example !== item.pattern && `/${example}` !== item.pattern).slice(0, 2);
    const rest = item.matches - shown.length;
    return [t.reason[item.reason], ...shown, shown.length > 0 && rest > 0 ? t.more(rest) : null].filter((part): part is string => Boolean(part)).join(m.common.dot);
  };

  return (
    <>
      {groups.map((group) => {
        const patterns = group.items.map((item) => item.pattern);
        const count = patterns.filter((pattern) => selected.has(pattern)).length;
        return (
          <div key={group.category} className={styles.group}>
            <Checkbox
              className={styles.groupHead}
              checked={count === 0 ? false : count === patterns.length ? true : "indeterminate"}
              onChange={(checked) => onToggle(patterns, checked)}
              label={t.category[group.category]}
            />
            {group.items.map((item) => (
              <Checkbox
                key={item.pattern}
                className={styles.item}
                checked={selected.has(item.pattern)}
                onChange={(checked) => onToggle([item.pattern], checked)}
                label={<span className="mono">{item.pattern}</span>}
                hint={describe(item)}
              />
            ))}
          </div>
        );
      })}
      {scan.truncated && <p className={styles.note}>{t.truncated}</p>}
    </>
  );
}

interface IgnorePickerProps {
  projects: Project[];
  auto: boolean;
  onAutoChange: (auto: boolean) => void;
  scans: Record<string, ScanState>;
  selection: IgnoreSelection;
  onToggle: (projectId: string, patterns: string[], checked: boolean) => void;
}

export function IgnorePicker({ projects, auto, onAutoChange, scans, selection, onToggle }: IgnorePickerProps) {
  const m = useT();
  const t = m.github.publish.ignore;
  const total = projects.reduce((sum, project) => sum + (selection[project.id]?.length ?? 0), 0);

  return (
    <section className={styles.picker}>
      <header className={styles.head}>
        <div className={styles.headText}>
          <p className={styles.title}>
            {t.title}
            {auto && total > 0 && <span className={styles.count}>{t.count(total)}</span>}
          </p>
          <p className={styles.hint}>{auto ? t.onHint : t.offHint}</p>
        </div>
        <Switch checked={auto} onChange={onAutoChange} ariaLabel={t.auto} />
      </header>
      {auto && (
        <div className={styles.list}>
          {projects.map((project) => {
            const state = scans[project.id];
            return (
              <div key={project.id} className={styles.project}>
                {projects.length > 1 && <p className={styles.projectName}>{project.name}</p>}
                {(!state || state.status === "loading") && (
                  <p className={styles.note}>
                    <Spinner size={12} />
                    {t.scanning}
                  </p>
                )}
                {state?.status === "failed" && <ErrorNote error={state.error} compact />}
                {state?.status === "ready" && (
                  <Suggestions scan={state.scan} selected={new Set(selection[project.id] ?? [])} onToggle={(patterns, checked) => onToggle(project.id, patterns, checked)} />
                )}
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
}
