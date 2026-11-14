import { useEffect } from "react";
import { Button } from "../../components/Button";
import { Dialog } from "../../components/Dialog";
import { ErrorNote } from "../../components/ErrorNote";
import { Icons } from "../../components/icons";
import { Spinner } from "../../components/Spinner";
import { useT } from "../../i18n";
import { api } from "../../ipc/client";
import { notifyError } from "../../lib/notify";
import { copyText } from "../../lib/platform";
import { useAppStore } from "../../stores/app";
import { useGithubStore } from "../../stores/github";
import { toast } from "../../stores/toasts";
import styles from "./dialogs.module.css";

export function LoginProgress({ onRetry }: { onRetry: () => void }) {
  const m = useT();
  const t = m.github.login;
  const login = useGithubStore((state) => state.login);

  if (login?.phase === "code") {
    return (
      <div className={styles.form}>
        <div className={styles.code}>
          <span className={styles.codeLabel}>{t.codeLabel}</span>
          <span className={`${styles.codeValue} selectable`}>{login.userCode}</span>
          <p className={styles.codeBody}>{t.codeBody}</p>
          <div className={styles.inline}>
            <Button size="sm" icon={<Icons.copy size={13} />} onClick={() => void copyText(login.userCode).then((ok) => ok && toast.success(m.common.copied))}>
              {t.copyCode}
            </Button>
            <Button size="sm" variant="primary" icon={<Icons.external size={13} />} onClick={() => void api.openUrl({ url: login.verificationUrl }).catch(notifyError)}>
              {t.openBrowser}
            </Button>
          </div>
        </div>
        <p className={styles.waiting}>
          <Spinner size={13} />
          {t.waiting}
        </p>
      </div>
    );
  }
  if (login?.phase === "failed") {
    return (
      <div className={styles.form}>
        <ErrorNote error={login.error} />
        <Button onClick={onRetry}>{m.common.retry}</Button>
      </div>
    );
  }
  return (
    <div className={styles.centered}>
      <Spinner size={18} />
      <span>{t.preparing}</span>
    </div>
  );
}

export function LoginDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const m = useT();
  const t = m.github.login;
  const status = useGithubStore((state) => state.status);
  const login = useGithubStore((state) => state.login);
  const startLogin = useGithubStore((state) => state.startLogin);
  const cancelLogin = useGithubStore((state) => state.cancelLogin);
  const loadStatus = useGithubStore((state) => state.loadStatus);
  const cliMissing = status !== null && !status.cliInstalled;
  const platform = useAppStore((state) => state.environment?.platform ?? "macos");

  useEffect(() => {
    if (open && !cliMissing && useGithubStore.getState().login === null) void startLogin();
  }, [open, cliMissing, startLogin]);

  useEffect(() => {
    if (open && login === null && status?.authenticated) onClose();
  }, [open, login, status?.authenticated, onClose]);

  const close = () => {
    void cancelLogin();
    onClose();
  };

  return (
    <Dialog open={open} onOpenChange={(next) => !next && close()} title={t.title} description={m.github.connect.body} size="sm">
      {cliMissing ? (
        <div className={styles.form}>
          <ErrorNote error={{ code: "gh_not_installed", message: "", detail: null }} />
          <p className={styles.hint}>{m.github.connect.cliMissingBody}</p>
          <code className={`${styles.command} selectable`}>{m.github.connect.install[platform]}</code>
          <Button onClick={() => void loadStatus(true)}>{m.github.connect.recheck}</Button>
        </div>
      ) : (
        <LoginProgress onRetry={() => void startLogin()} />
      )}
    </Dialog>
  );
}
