import type { Messages } from "../../i18n";
import type { Divergence, OperationRequest, PreferSide, Project, SyncState } from "../../ipc/types";
import { displayPath } from "../../lib/path";
import type { SortKey, SummaryFilter } from "../../stores/app";

const ATTENTION: SyncState[] = [
  "conflicted",
  "in_progress",
  "detached",
  "remote_not_found",
  "auth_error",
  "unreachable",
  "diverged",
  "missing",
  "unknown",
];
const UNPUBLISHED: SyncState[] = ["not_initialized", "no_remote", "unpublished"];

const stateOf = (project: Project): SyncState => project.status?.state ?? "unknown";

export function targetInvalid(project: Project): boolean {
  const targets = project.status?.targets;
  return Boolean(targets?.push.savedInvalid || targets?.pull.savedInvalid);
}

export function matchesFilter(project: Project, filter: SummaryFilter): boolean {
  const status = project.status;
  if (filter === "all") return true;
  if (!status) return false;
  switch (filter) {
    case "synced":
      return status.state === "synced";
    case "dirty":
      return status.changes.total > 0 && status.state !== "conflicted";
    case "ahead":
      return (status.ahead ?? 0) > 0;
    case "behind":
      return (status.behind ?? 0) > 0;
    case "unpublished":
      return UNPUBLISHED.includes(status.state);
    case "attention":
      return ATTENTION.includes(status.state) || targetInvalid(project);
  }
}

export const SUMMARY_FILTERS: SummaryFilter[] = ["all", "synced", "dirty", "ahead", "behind", "unpublished", "attention"];

export function countSummary(projects: Project[]): Record<SummaryFilter, number> {
  const counts = { all: 0, synced: 0, dirty: 0, ahead: 0, behind: 0, unpublished: 0, attention: 0 };
  for (const project of projects) {
    for (const filter of SUMMARY_FILTERS) if (matchesFilter(project, filter)) counts[filter] += 1;
  }
  return counts;
}

export function matchesQuery(project: Project, query: string): boolean {
  const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return true;
  const haystack = [
    project.name,
    displayPath(project.path),
    project.status?.branch ?? "",
    project.status?.github ? `${project.status.github.owner}/${project.status.github.repo}` : "",
  ]
    .join("\n")
    .toLowerCase();
  return words.every((word) => haystack.includes(word));
}

const STATE_RANK: Record<SyncState, number> = {
  conflicted: 0,
  in_progress: 1,
  diverged: 2,
  auth_error: 3,
  remote_not_found: 4,
  unreachable: 5,
  missing: 6,
  unknown: 7,
  detached: 8,
  dirty: 9,
  behind: 10,
  ahead: 11,
  unpublished: 12,
  no_remote: 13,
  not_initialized: 14,
  synced: 15,
};

const collator = new Intl.Collator("ja", { numeric: true, sensitivity: "base" });

export function sortProjects(projects: Project[], key: SortKey, dir: "asc" | "desc", favoritesFirst: boolean): Project[] {
  const sign = dir === "asc" ? 1 : -1;
  const compare = (a: Project, b: Project): number => {
    switch (key) {
      case "state":
        return (STATE_RANK[stateOf(a)] - STATE_RANK[stateOf(b)]) * sign || collator.compare(a.name, b.name);
      case "updated":
        return ((b.status?.lastCommitAt ?? 0) - (a.status?.lastCommitAt ?? 0)) * sign || collator.compare(a.name, b.name);
      case "path":
        return collator.compare(a.path, b.path) * sign;
      default:
        return collator.compare(a.name, b.name) * sign || collator.compare(a.path, b.path);
    }
  };
  return [...projects].sort((a, b) => {
    if (favoritesFirst && a.settings.favorite !== b.settings.favorite) return a.settings.favorite ? -1 : 1;
    return compare(a, b);
  });
}

export type PrimaryAction =
  | { type: "operation"; label: string; hint: string; request: OperationRequest; emphasis: boolean }
  | { type: "open"; label: string; hint: string; tab: "changes" | "branches" | "settings"; emphasis: boolean }
  | { type: "dialog"; label: string; hint: string; dialog: "publish" | "diverged" | "target" | "login"; emphasis: boolean }
  | { type: "refresh"; label: string; hint: string; emphasis: boolean }
  | null;

export function newerSide(divergence: Divergence | null | undefined): PreferSide | null {
  if (!divergence?.localAt || !divergence.remoteAt) return null;
  if (Math.abs(divergence.localAt - divergence.remoteAt) < 60_000) return null;
  return divergence.localAt > divergence.remoteAt ? "local" : "remote";
}

export function primaryAction(project: Project, m: Messages): PrimaryAction {
  const status = project.status;
  const base = { itemId: project.id, projectId: project.id };
  if (!status) return { type: "refresh", label: m.common.refresh, hint: m.action.refresh, emphasis: false };
  if (targetInvalid(project)) {
    return { type: "dialog", dialog: "target", label: m.action.chooseTarget, hint: m.guidance.targetInvalid, emphasis: true };
  }
  switch (status.state) {
    case "synced":
      return { type: "operation", label: m.action.sync, hint: m.actionHint.sync, request: { kind: "sync", items: [base] }, emphasis: false };
    case "dirty":
      return { type: "open", tab: "changes", label: m.action.review, hint: m.guidance.dirty, emphasis: true };
    case "ahead":
      return {
        type: "operation",
        label: m.action.push,
        hint: m.actionHint.push,
        request: { kind: "push", items: [{ ...base, force: false, setUpstream: false }] },
        emphasis: true,
      };
    case "behind":
      return {
        type: "operation",
        label: m.action.pull,
        hint: m.actionHint.pull,
        request: { kind: "pull", items: [{ ...base, mode: "ff_only" }] },
        emphasis: true,
      };
    case "diverged":
      return { type: "dialog", dialog: "diverged", label: m.action.pull, hint: m.guidance.diverged, emphasis: true };
    case "conflicted":
    case "in_progress":
      return { type: "open", tab: "changes", label: m.action.resolve, hint: m.guidance.conflicted, emphasis: true };
    case "detached":
      return { type: "open", tab: "branches", label: m.branches.switch, hint: m.guidance.detached, emphasis: false };
    case "no_remote":
      return { type: "dialog", dialog: "publish", label: m.action.publish, hint: m.guidance.no_remote, emphasis: false };
    case "not_initialized":
      return { type: "open", tab: "changes", label: m.action.setup, hint: m.guidance.not_initialized, emphasis: false };
    case "remote_not_found":
      return { type: "open", tab: "settings", label: m.common.details, hint: m.guidance.remote_not_found, emphasis: false };
    case "auth_error":
      return { type: "dialog", dialog: "login", label: m.action.reconnect, hint: m.guidance.auth_error, emphasis: true };
    case "unreachable":
      return { type: "operation", label: m.common.retry, hint: m.guidance.unreachable, request: { kind: "fetch", items: [base] }, emphasis: false };
    case "unpublished":
      return status.targets.push.target
        ? {
            type: "operation",
            label: m.action.push,
            hint: m.guidance.unpublished,
            request: { kind: "push", items: [{ ...base, force: false, setUpstream: false }] },
            emphasis: true,
          }
        : { type: "dialog", dialog: "target", label: m.action.chooseTarget, hint: m.guidance.unpublishedUnresolved, emphasis: true };
    case "missing":
      return null;
    case "unknown":
      return { type: "refresh", label: m.common.retry, hint: m.guidance.unknown, emphasis: false };
  }
}

export function batchRequest(kind: "fetch" | "pull" | "push" | "sync", projectIds: string[]): OperationRequest {
  switch (kind) {
    case "fetch":
      return { kind, items: projectIds.map((id) => ({ itemId: id, projectId: id })) };
    case "sync":
      return { kind, items: projectIds.map((id) => ({ itemId: id, projectId: id })) };
    case "pull":
      return { kind, items: projectIds.map((id) => ({ itemId: id, projectId: id, mode: "ff_only" as const })) };
    case "push":
      return { kind, items: projectIds.map((id) => ({ itemId: id, projectId: id, force: false, setUpstream: false })) };
  }
}

export const remoteLabel = (ref: { remote: string; branch: string }) => `${ref.remote}/${ref.branch}`;
