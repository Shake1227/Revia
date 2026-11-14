import { create } from "zustand";

export type ToastKind = "success" | "error" | "warning" | "info";

export interface Toast {
  id: string;
  kind: ToastKind;
  title: string;
  description?: string;
  detail?: string | null;
  action?: { label: string; run: () => void };
  duration: number;
}

interface ToastStore {
  toasts: Toast[];
  push(toast: Omit<Toast, "id" | "duration"> & { duration?: number }): string;
  dismiss(id: string): void;
}

let counter = 0;
const MAX_VISIBLE = 4;

export const useToastStore = create<ToastStore>((set) => ({
  toasts: [],
  push(toast) {
    counter += 1;
    const id = `toast-${counter}`;
    const duration = toast.duration ?? (toast.kind === "error" ? 9000 : toast.action ? 7000 : 3600);
    set((state) => ({ toasts: [...state.toasts, { ...toast, id, duration }].slice(-MAX_VISIBLE) }));
    return id;
  },
  dismiss(id) {
    set((state) => ({ toasts: state.toasts.filter((toast) => toast.id !== id) }));
  },
}));

export const toast = {
  success: (title: string, extra: Partial<Toast> = {}) => useToastStore.getState().push({ kind: "success", title, ...extra }),
  error: (title: string, extra: Partial<Toast> = {}) => useToastStore.getState().push({ kind: "error", title, ...extra }),
  warning: (title: string, extra: Partial<Toast> = {}) => useToastStore.getState().push({ kind: "warning", title, ...extra }),
  info: (title: string, extra: Partial<Toast> = {}) => useToastStore.getState().push({ kind: "info", title, ...extra }),
};
