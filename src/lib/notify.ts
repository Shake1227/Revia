import { t } from "../i18n";
import { toAppError } from "../ipc/errors";
import { toast } from "../stores/toasts";

export function notifyError(error: unknown, subject?: string) {
  const appError = toAppError(error);
  const text = t().error[appError.code] ?? t().error.unknown;
  toast.error(subject ? `${subject}: ${text.title}` : text.title, {
    description: text.hint || undefined,
    detail: appError.detail,
  });
  return appError;
}
