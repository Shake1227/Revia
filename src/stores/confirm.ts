import type { ReactNode } from "react";
import { create } from "zustand";
import type { RiskLevel } from "../ipc/types";

export interface ConfirmFact {
  label: string;
  value: ReactNode;
}

export interface ConfirmRequest {
  title: string;
  body?: ReactNode;
  facts?: ConfirmFact[];
  risk?: ReactNode;
  level: RiskLevel;
  confirmLabel: string;
  cancelLabel?: string;
  typeToConfirm?: string;
  acknowledge?: boolean;
  alternative?: { label: string; value: string };
  options?: { id: string; label: string; hint?: string; defaultChecked?: boolean }[];
  content?: ReactNode;
}

export interface ConfirmResult {
  confirmed: boolean;
  choice: string | null;
  options: Record<string, boolean>;
}

interface ConfirmStore {
  current: (ConfirmRequest & { resolve: (result: ConfirmResult) => void }) | null;
  ask(request: ConfirmRequest): Promise<ConfirmResult>;
  settle(result: ConfirmResult): void;
}

export const useConfirmStore = create<ConfirmStore>((set, get) => ({
  current: null,
  ask(request) {
    get().current?.resolve({ confirmed: false, choice: null, options: {} });
    return new Promise<ConfirmResult>((resolve) => set({ current: { ...request, resolve } }));
  },
  settle(result) {
    const current = get().current;
    if (!current) return;
    set({ current: null });
    current.resolve(result);
  },
}));

export const confirm = (request: ConfirmRequest) => useConfirmStore.getState().ask(request);
