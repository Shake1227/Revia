import { create } from "zustand";
import { t } from "../i18n";
import { api } from "../ipc/client";
import { toAppError } from "../ipc/errors";
import type {
  AppError,
  ItemState,
  OperationEvent,
  OperationItem,
  OperationKind,
  OperationPlan,
  OperationRequest,
  OperationSnapshot,
  PlanBlocker,
  PlanItem,
  StepCode,
} from "../ipc/types";
import { useAppStore } from "./app";
import { getProject } from "./projects";
import { toast } from "./toasts";
import { useUiStore } from "./ui";

export interface Activity {
  operationId: string | null;
  kind: OperationKind;
  state: "queued" | "running";
  step: StepCode | null;
  progress: number | null;
}

export type Sheet =
  | { phase: "plan"; request: OperationRequest; plan: OperationPlan | null; error: AppError | null; starting: boolean }
  | { phase: "run"; operationId: string };

interface OperationStore {
  operations: Record<string, OperationSnapshot>;
  quiet: Record<string, true>;
  silent: Record<string, true>;
  labels: Record<string, string>;
  sheet: Sheet | null;
  activity: Record<string, Activity>;
  flash: Record<string, { state: ItemState; at: number }>;

  prepare(request: OperationRequest): Promise<void>;
  execute(transform?: (request: OperationRequest) => OperationRequest): Promise<void>;
  quick(request: OperationRequest): Promise<void>;
  run(request: OperationRequest, plan?: OperationPlan | null): Promise<OperationSnapshot | null>;
  adopt(snapshot: OperationSnapshot, mode: "silent" | "quiet" | "batch"): void;
  cancel(operationId: string): Promise<void>;
  showOperation(operationId: string): void;
  closeSheet(): void;
  handleEvent(event: OperationEvent): void;
}

const TERMINAL: ItemState[] = ["succeeded", "skipped", "failed", "needs_confirmation", "conflict", "cancelled"];
const isTerminal = (state: ItemState) => TERMINAL.includes(state);
const QUIET_WARNINGS = new Set(["uncommitted_changes", "stale_remote"]);

export function isSimplePlan(plan: OperationPlan): boolean {
  return plan.items.every(
    (item) => item.executable && item.risk === "safe" && item.warnings.every((warning) => QUIET_WARNINGS.has(warning.code)),
  );
}

export function executableRequest(request: OperationRequest, plan: OperationPlan): OperationRequest {
  const byId = new Map(plan.items.map((item) => [item.itemId, item]));
  const items = request.items
    .filter((item) => byId.get(item.itemId)?.executable)
    .map((item) => {
      const planned = byId.get(item.itemId);
      return { ...item, fingerprint: planned && planned.risk !== "safe" ? planned.fingerprint : null };
    });
  return { ...request, items } as OperationRequest;
}

export function resolveBlocker(projectId: string, blocker: PlanBlocker | null) {
  const ui = useUiStore.getState();
  switch (blocker) {
    case "diverged":
      return ui.openDialog({ type: "diverged", projectId });
    case "no_remote":
      return ui.openDialog({ type: "publish", projectIds: [projectId] });
    case "target_unresolved":
    case "target_invalid":
      return ui.openDialog({ type: "target", projectId });
    case "not_authenticated":
      return ui.openDialog({ type: "login" });
    case "detached":
      return ui.openProject(projectId, "branches");
    case "feature_disabled":
      return ui.navigate({ name: "settings" });
    default:
      return ui.openProject(projectId, "changes");
  }
}

const projectName = (item: { projectId: string | null }) => (item.projectId ? (getProject(item.projectId)?.name ?? "") : "");

function announceSingle(kind: OperationKind, item: OperationItem) {
  const m = t();
  const name = projectName(item);
  const ui = useAppStore.getState().ui;
  switch (item.state) {
    case "succeeded": {
      const pulled = item.completed.find((step) => step.code === "pull");
      const pushed = item.completed.find((step) => step.code === "push");
      if (kind === "sync") {
        toast.success(pulled || pushed ? m.ops.toast.syncDone(name) : m.ops.toast.syncUpToDate(name));
      } else if (kind === "fetch") toast.success(m.ops.toast.fetchDone(name));
      else if (kind === "pull") toast.success(m.ops.toast.pullDone(name, pulled?.code === "pull" ? pulled.commits : 0));
      else if (kind === "push") toast.success(m.ops.toast.pushDone(name, pushed?.code === "push" ? pushed.commits : null));
      else if (kind === "commit") toast.success(m.ops.toast.commitDone(name));
      else if (kind === "merge") toast.success(m.ops.toast.mergeDone(name));
      else if (kind === "publish") toast.success(m.ops.toast.publishDone(name));
      else if (kind === "clone") toast.success(m.ops.toast.cloneDone(name));
      return;
    }
    case "skipped":
      toast.info(m.ops.nothingToDo[kind] === m.ops.nothingToDo.sync ? m.ops.toast.nothing(name) : `${name}: ${m.ops.nothingToDo[kind]}`);
      return;
    case "needs_confirmation": {
      const projectId = item.projectId;
      toast.warning(m.ops.toast.needsConfirmation(name), {
        description: item.reason ? m.ops.blocker[item.reason] : undefined,
        action: projectId ? { label: m.ops.confirmInProject, run: () => resolveBlocker(projectId, item.reason) } : undefined,
      });
      return;
    }
    case "conflict": {
      if (!ui.notifyErrors) return;
      const projectId = item.projectId;
      toast.warning(m.ops.toast.conflict(name), {
        description: m.error.merge_conflict.hint,
        action: projectId ? { label: m.action.resolve, run: () => useUiStore.getState().openProject(projectId, "changes") } : undefined,
        duration: 9000,
      });
      return;
    }
    case "failed": {
      if (!ui.notifyErrors) return;
      const error = item.error;
      const text = error ? m.error[error.code] : m.error.unknown;
      toast.error(name ? `${name}: ${text.title}` : text.title, {
        description: text.hint || undefined,
        detail: error?.detail,
      });
      return;
    }
    case "cancelled":
      toast.info(m.ops.cancelled);
      return;
    default:
      return;
  }
}

function announceBatch(snapshot: OperationSnapshot) {
  const m = t();
  const ui = useAppStore.getState().ui;
  const title = m.ops.title[snapshot.kind](snapshot.items.length);
  const issues = snapshot.items.filter((item) => ["failed", "needs_confirmation", "conflict"].includes(item.state)).length;
  const show = { label: m.common.details, run: () => useOperationStore.getState().showOperation(snapshot.operationId) };
  if (issues > 0) {
    if (ui.notifyErrors) toast.warning(m.ops.toast.batchIssues(title, issues), { action: show, duration: 9000 });
  } else if (ui.notifyOperationDone) {
    toast.success(m.ops.toast.batchDone(title), { action: show });
  }
}

const flashTimers = new Map<string, ReturnType<typeof setTimeout>>();

export const useOperationStore = create<OperationStore>((set, get) => {
  const setFlash = (projectId: string, state: ItemState) => {
    set((store) => ({ flash: { ...store.flash, [projectId]: { state, at: Date.now() } } }));
    clearTimeout(flashTimers.get(projectId));
    flashTimers.set(
      projectId,
      setTimeout(() => {
        set((store) => {
          const flash = { ...store.flash };
          delete flash[projectId];
          return { flash };
        });
      }, 2400),
    );
  };

  const applyItem = (operationId: string, kind: OperationKind, item: OperationItem) => {
    if (!item.projectId) return;
    const projectId = item.projectId;
    if (item.state === "queued" || item.state === "running") {
      set((store) => ({
        activity: {
          ...store.activity,
          [projectId]: { operationId, kind, state: item.state as "queued" | "running", step: item.step, progress: item.progress },
        },
      }));
      return;
    }
    const current = get().activity[projectId];
    if (current && (current.operationId === operationId || current.operationId === null)) {
      set((store) => {
        const activity = { ...store.activity };
        delete activity[projectId];
        return { activity };
      });
      setFlash(projectId, item.state);
    }
  };

  const register = (snapshot: OperationSnapshot) => {
    set((store) => ({ operations: { ...store.operations, [snapshot.operationId]: snapshot } }));
    for (const item of snapshot.items) applyItem(snapshot.operationId, snapshot.kind, item);
  };

  const rememberLabels = (request: OperationRequest) => {
    if (request.kind !== "clone") return;
    set((store) => ({
      labels: { ...store.labels, ...Object.fromEntries(request.items.map((item) => [item.itemId, item.folderName])) },
    }));
  };

  const setPending = (request: OperationRequest, pending: boolean) => {
    set((store) => {
      const activity = { ...store.activity };
      for (const item of request.items) {
        if (!("projectId" in item)) continue;
        if (pending) activity[item.projectId] = { operationId: null, kind: request.kind, state: "queued", step: null, progress: null };
        else if (activity[item.projectId]?.operationId === null) delete activity[item.projectId];
      }
      return { activity };
    });
  };

  const fail = (error: unknown) => {
    const appError = toAppError(error);
    const text = t().error[appError.code] ?? t().error.unknown;
    toast.error(text.title, { description: text.hint || undefined, detail: appError.detail });
  };

  return {
    operations: {},
    quiet: {},
    silent: {},
    labels: {},
    sheet: null,
    activity: {},
    flash: {},

    async prepare(request) {
      set({ sheet: { phase: "plan", request, plan: null, error: null, starting: false } });
      try {
        const plan = await api.planOperation({ request });
        const sheet = get().sheet;
        if (sheet?.phase === "plan" && sheet.request === request) set({ sheet: { ...sheet, plan } });
      } catch (error) {
        const sheet = get().sheet;
        if (sheet?.phase === "plan" && sheet.request === request) set({ sheet: { ...sheet, error: toAppError(error) } });
      }
    },

    async execute(transform) {
      const sheet = get().sheet;
      if (sheet?.phase !== "plan" || !sheet.plan || sheet.starting) return;
      const request = executableRequest(transform ? transform(sheet.request) : sheet.request, sheet.plan);
      if (request.items.length === 0) return;
      set({ sheet: { ...sheet, starting: true } });
      rememberLabels(request);
      try {
        const snapshot = await api.startOperation({ request });
        register(snapshot);
        set({ sheet: { phase: "run", operationId: snapshot.operationId } });
      } catch (error) {
        set({ sheet: { ...sheet, starting: false, error: toAppError(error) } });
      }
    },

    async quick(request) {
      setPending(request, true);
      try {
        const plan = await api.planOperation({ request });
        if (isSimplePlan(plan)) {
          const snapshot = await api.startOperation({ request: executableRequest(request, plan) });
          set((store) => ({ quiet: { ...store.quiet, [snapshot.operationId]: true } }));
          register(snapshot);
          return;
        }
        setPending(request, false);
        const only: PlanItem | undefined = plan.items.length === 1 ? plan.items[0] : undefined;
        if (only && !only.executable && only.projectId) {
          if (only.blocker === "nothing_to_do") {
            toast.info(`${getProject(only.projectId)?.name ?? ""}: ${t().ops.nothingToDo[request.kind]}`);
            return;
          }
          const hasFlow = ["diverged", "no_remote", "target_unresolved", "target_invalid", "not_authenticated"].includes(only.blocker ?? "");
          if (!hasFlow && only.blocker) toast.warning(t().ops.blocker[only.blocker], { description: getProject(only.projectId)?.name });
          resolveBlocker(only.projectId, only.blocker);
          return;
        }
        set({ sheet: { phase: "plan", request, plan, error: null, starting: false } });
      } catch (error) {
        setPending(request, false);
        fail(error);
      }
    },

    async run(request, plan = null) {
      setPending(request, true);
      rememberLabels(request);
      try {
        const snapshot = await api.startOperation({ request: plan ? executableRequest(request, plan) : request });
        set((store) => ({ quiet: { ...store.quiet, [snapshot.operationId]: true } }));
        register(snapshot);
        return snapshot;
      } catch (error) {
        setPending(request, false);
        fail(error);
        return null;
      }
    },

    adopt(snapshot, mode) {
      if (mode === "silent") set((store) => ({ silent: { ...store.silent, [snapshot.operationId]: true } }));
      if (mode === "quiet") set((store) => ({ quiet: { ...store.quiet, [snapshot.operationId]: true } }));
      register(snapshot);
    },

    async cancel(operationId) {
      try {
        await api.cancelOperation({ operationId });
      } catch (error) {
        fail(error);
      }
    },

    showOperation(operationId) {
      if (get().operations[operationId]) set({ sheet: { phase: "run", operationId } });
    },

    closeSheet() {
      set({ sheet: null });
    },

    handleEvent(event) {
      const existing = get().operations[event.operationId];
      if (event.kind === "state") {
        const wasFinished = existing?.state === "finished";
        register(event.snapshot);
        if (event.snapshot.state === "finished" && !wasFinished) {
          const sheet = get().sheet;
          const visible = sheet?.phase === "run" && sheet.operationId === event.operationId;
          const isQuiet = get().quiet[event.operationId];
          if (get().silent[event.operationId]) return;
          const changed = event.snapshot.items.some((item) => item.completed.some((step) => step.code !== "fetch"));
          if (changed && event.snapshot.items.every((item) => item.state === "succeeded" || item.state === "skipped")) {
            useUiStore.getState().celebrate();
          }
          if (isQuiet && event.snapshot.items.length === 1) announceSingle(event.snapshot.kind, event.snapshot.items[0]!);
          else if (!visible) announceBatch(event.snapshot);
        }
        return;
      }
      if (!existing) {
        void api.getOperation({ operationId: event.operationId }).then((snapshot) => snapshot && register(snapshot));
        return;
      }
      const items = existing.items.map((item) => (item.itemId === event.item.itemId ? event.item : item));
      set((store) => ({ operations: { ...store.operations, [event.operationId]: { ...existing, items } } }));
      applyItem(event.operationId, existing.kind, event.item);
    },
  };
});

export const runningOperations = (operations: Record<string, OperationSnapshot>) =>
  Object.values(operations).filter((operation) => operation.state !== "finished");

export { isTerminal };

export function waitForOperation(operationId: string): Promise<OperationSnapshot> {
  return new Promise((resolve) => {
    const check = (operations: Record<string, OperationSnapshot>) => {
      const snapshot = operations[operationId];
      if (snapshot?.state === "finished") {
        unsubscribe();
        resolve(snapshot);
        return true;
      }
      return false;
    };
    const unsubscribe = useOperationStore.subscribe((state) => void check(state.operations));
    check(useOperationStore.getState().operations);
  });
}

export async function runAndWait(request: OperationRequest, plan: OperationPlan | null = null): Promise<OperationSnapshot | null> {
  const started = await useOperationStore.getState().run(request, plan);
  return started ? waitForOperation(started.operationId) : null;
}
