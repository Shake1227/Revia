import clsx from "clsx";
import { useMemo, useState } from "react";
import { Button } from "../../components/Button";
import { EmptyState } from "../../components/EmptyState";
import { SearchField } from "../../components/Field";
import { Icons } from "../../components/icons";
import { Tooltip } from "../../components/Tooltip";
import { useT } from "../../i18n";
import { api } from "../../ipc/client";
import { notifyError } from "../../lib/notify";
import { displayPath } from "../../lib/path";
import { useAppStore } from "../../stores/app";
import { useUiStore } from "../../stores/ui";
import { PageHeader } from "../shell/PageHeader";
import styles from "./ExcludedPage.module.css";

export function ExcludedPage() {
  const m = useT();
  const t = m.excluded;
  const paths = useAppStore((state) => state.settings?.excludedPaths ?? []);
  const applyWorkspace = useAppStore((state) => state.applyWorkspace);
  const back = useUiStore((state) => state.back);
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState<string | null>(null);

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const sorted = [...paths].sort((a, b) => a.localeCompare(b));
    return needle ? sorted.filter((path) => path.toLowerCase().includes(needle)) : sorted;
  }, [paths, query]);

  const restore = async (path: string) => {
    setBusy(path);
    try {
      applyWorkspace(await api.restoreExcludedPath({ path }));
    } catch (error) {
      notifyError(error);
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className={styles.page}>
      <div className={styles.bar}>
        <Tooltip label={m.common.back} shortcut={["esc"]}>
          <button type="button" className={styles.back} onClick={back} aria-label={m.common.back}>
            <Icons.chevronLeft size={16} />
            <span>{m.nav.settings}</span>
          </button>
        </Tooltip>
      </div>
      <PageHeader
        title={t.title}
        meta={paths.length > 0 ? m.common.items(paths.length) : null}
        subtitle={t.subtitle}
        actions={
          paths.length > 0 ? (
            <SearchField value={query} onChange={setQuery} placeholder={t.search} clearLabel={m.common.close} className={styles.search} />
          ) : undefined
        }
      />
      <div className={clsx(styles.scroll, "rv-sheet")}>
        {paths.length === 0 ? (
          <EmptyState icon={<Icons.archive size={22} />} title={t.empty} body={t.emptyBody} />
        ) : visible.length === 0 ? (
          <EmptyState compact icon={<Icons.search size={20} />} title={t.noMatch} />
        ) : (
          <ul className={styles.list}>
            {visible.map((path) => (
              <li key={path} className={styles.row}>
                <span className={styles.icon}>
                  <Icons.archive size={14} />
                </span>
                <div className={styles.main}>
                  <span className={clsx("truncate", styles.name)}>{path.split(/[\\/]/).filter(Boolean).pop() ?? path}</span>
                  <span className={clsx("mono selectable truncate", styles.path)}>{displayPath(path)}</span>
                </div>
                <Button size="sm" variant="ghost" icon={<Icons.undo size={13} />} loading={busy === path} disabled={busy !== null} onClick={() => void restore(path)}>
                  {t.restore}
                </Button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
