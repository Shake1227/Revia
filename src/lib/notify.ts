import { t } from "../i18n";
import { toAppError } from "../ipc/errors";
import { useGithubStore } from "../stores/github";
import { toast } from "../stores/toasts";

export function notifyError(error: unknown, subject?: string) {
  const appError = toAppError(error);
  const text = t().error[appError.code] ?? t().error.unknown;
  toast.error(subject ? `${subject}: ${text.title}` : text.title, {
    description: text.hint || undefined,
    detail: appError.detail,
    action:
      appError.code === "gh_not_installed"
        ? { label: t().github.connect.installAction, run: () => void useGithubStore.getState().installCli() }
        : undefined,
  });
  return appError;
}
