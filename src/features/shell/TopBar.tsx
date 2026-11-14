import clsx from "clsx";
import { Avatar } from "../../components/Avatar";
import { Icons } from "../../components/icons";
import { Kbd } from "../../components/Kbd";
import { LogoMark, Wordmark } from "../../components/Logo";
import { Spinner } from "../../components/Spinner";
import { Tooltip } from "../../components/Tooltip";
import { useT } from "../../i18n";
import { useGithubStore } from "../../stores/github";
import { runningOperations, useOperationStore } from "../../stores/operations";
import { useUiStore } from "../../stores/ui";
import styles from "./TopBar.module.css";

export function TopBar({ inset, controls, tone }: { inset: boolean; controls: boolean; tone: "light" | "dark" | null }) {
  const m = useT();
  const setPaletteOpen = useUiStore((state) => state.setPaletteOpen);
  const navigate = useUiStore((state) => state.navigate);
  const openDialog = useUiStore((state) => state.openDialog);
  const github = useGithubStore((state) => state.status);
  const operations = useOperationStore((state) => state.operations);
  const showOperation = useOperationStore((state) => state.showOperation);

  const running = runningOperations(operations);
  const total = running.reduce((sum, operation) => sum + operation.items.length, 0);
  const done = running.reduce(
    (sum, operation) => sum + operation.items.filter((item) => item.state !== "queued" && item.state !== "running").length,
    0,
  );

  return (
    <header
      className={styles.topbar}
      data-inset={inset || undefined}
      data-controls={controls || undefined}
      data-sky={tone ?? undefined}
      data-tauri-drag-region
    >
      <div className={styles.brand} data-tauri-drag-region>
        <LogoMark size={22} />
        <Wordmark />
      </div>

      <button type="button" className={styles.command} data-surface onClick={() => setPaletteOpen(true)} aria-label={m.nav.palette}>
        <Icons.search size={14} />
        <span className={styles.commandLabel}>{m.palette.placeholder}</span>
        <Kbd keys={["mod", "K"]} />
      </button>

      <div className={styles.right} data-tauri-drag-region>
        {running.length > 0 && (
          <Tooltip label={m.ops.show}>
            <button type="button" className={styles.activity} data-surface onClick={() => showOperation(running[0]!.operationId)}>
              <Spinner size={13} />
              <span className={styles.activityLabel}>{m.ops.title[running[0]!.kind](total)}</span>
              <span className="tabular">
                {done}/{total}
              </span>
            </button>
          </Tooltip>
        )}
        {github?.authenticated && github.account ? (
          <Tooltip label={`GitHub: ${github.account.login}`}>
            <button type="button" className={styles.account} data-surface onClick={() => navigate({ name: "github" })}>
              <Avatar name={github.account.login} src={github.account.avatarUrl} size={22} />
              <span className={clsx(styles.accountName, "truncate")}>{github.account.login}</span>
            </button>
          </Tooltip>
        ) : (
          <button type="button" className={clsx(styles.account, styles.connect)} onClick={() => openDialog({ type: "login" })}>
            <Icons.github size={14} />
            <span className={styles.accountName}>{m.nav.connectGithub}</span>
          </button>
        )}
      </div>
    </header>
  );
}
