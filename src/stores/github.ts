import { create } from "zustand";
import { t } from "../i18n";
import { api } from "../ipc/client";
import { toAppError } from "../ipc/errors";
import type { AppError, GithubLoginEvent, GithubOwner, GithubRepo, GithubStatus } from "../ipc/types";
import { toast } from "./toasts";

export type LoginFlow =
  | { phase: "starting"; loginId: string | null }
  | { phase: "code"; loginId: string; userCode: string; verificationUrl: string }
  | { phase: "failed"; error: AppError };

interface GithubStore {
  status: GithubStatus | null;
  statusLoading: boolean;
  statusError: AppError | null;
  repos: GithubRepo[] | null;
  reposLoading: boolean;
  reposError: AppError | null;
  owners: GithubOwner[] | null;
  login: LoginFlow | null;
  addingScope: boolean;
  installing: boolean;

  loadStatus(refresh?: boolean): Promise<void>;
  installCli(): Promise<void>;
  loadRepos(refresh?: boolean): Promise<void>;
  loadOwners(): Promise<void>;
  startLogin(scopes?: string[]): Promise<void>;
  cancelLogin(): Promise<void>;
  logout(): Promise<void>;
  handleLoginEvent(event: GithubLoginEvent): void;
}

export const useGithubStore = create<GithubStore>((set, get) => ({
  status: null,
  statusLoading: false,
  statusError: null,
  repos: null,
  reposLoading: false,
  reposError: null,
  owners: null,
  login: null,
  addingScope: false,
  installing: false,

  async loadStatus(refresh = false) {
    set({ statusLoading: true });
    try {
      const previous = get().status;
      const status = await api.githubStatus({ refresh });
      set({ status, statusError: null });
      if (previous?.authenticated && !status.authenticated) {
        set({ repos: null, owners: null });
        toast.warning(t().github.account.expired);
      }
    } catch (error) {
      set({ statusError: toAppError(error) });
    } finally {
      set({ statusLoading: false });
    }
  },

  async installCli() {
    if (get().installing) return;
    set({ installing: true });
    try {
      const status = await api.installGithubCli();
      set({ status });
      if (status.cliInstalled) toast.success(t().github.connect.installed);
    } catch (error) {
      const appError = toAppError(error);
      const text = t().error[appError.code] ?? t().error.unknown;
      toast.error(t().github.connect.installFailed, { description: text.title, detail: appError.detail });
    } finally {
      set({ installing: false });
    }
  },

  async loadRepos(refresh = false) {
    if (get().reposLoading) return;
    set({ reposLoading: true, reposError: null });
    try {
      set({ repos: await api.githubListRepos({ refresh }) });
    } catch (error) {
      const appError = toAppError(error);
      set({ reposError: appError });
      if (appError.code === "not_authenticated" || appError.code === "auth_failed") void get().loadStatus(true);
    } finally {
      set({ reposLoading: false });
    }
  },

  async loadOwners() {
    try {
      set({ owners: await api.githubListOwners() });
    } catch {
      set({ owners: [] });
    }
  },

  async startLogin(scopes) {
    set({ login: { phase: "starting", loginId: null }, addingScope: scopes !== undefined });
    try {
      const { loginId } = await api.githubLoginStart({ scopes: scopes ?? null });
      const current = get().login;
      if (current?.phase === "starting") set({ login: { phase: "starting", loginId } });
    } catch (error) {
      set({ login: { phase: "failed", error: toAppError(error) } });
    }
  },

  async cancelLogin() {
    const login = get().login;
    set({ login: null });
    if (login && login.phase !== "failed" && login.loginId) {
      await api.githubLoginCancel({ loginId: login.loginId }).catch(() => undefined);
    }
  },

  async logout() {
    const status = await api.githubLogout();
    set({ status, repos: null, owners: null });
  },

  handleLoginEvent(event) {
    if (!get().login) return;
    switch (event.kind) {
      case "code":
        set({ login: { phase: "code", loginId: event.loginId, userCode: event.userCode, verificationUrl: event.verificationUrl } });
        return;
      case "succeeded":
        set({ login: null, status: event.status, repos: null });
        toast.success(get().addingScope ? t().removal.delete.scopeAdded : t().github.login.success(event.status.account?.login ?? ""));
        void get().loadRepos(true);
        return;
      case "failed":
        set({ login: { phase: "failed", error: event.error } });
        return;
      case "cancelled":
        set({ login: null });
        return;
    }
  },
}));
