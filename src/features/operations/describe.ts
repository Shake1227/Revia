import type { Messages } from "../../i18n";
import type { OperationItem, OperationKind, PlanStep, PlanWarning } from "../../ipc/types";
import { remoteLabel } from "../projects/model";

export function stepChip(step: PlanStep, m: Messages): string {
  switch (step.code) {
    case "pull":
      return step.commits > 0 ? `${m.ops.step.pull} ${step.commits}` : m.ops.step.pull;
    case "push":
      return step.commits ? `${m.ops.step.push} ${step.commits}` : m.ops.step.push;
    case "commit":
      return `${m.ops.step.commit} ${step.files}`;
    case "merge":
      return step.commits > 0 ? `${m.ops.step.merge} ${step.commits}` : m.ops.step.merge;
    default:
      return m.ops.step[step.code];
  }
}

export function stepDetail(step: PlanStep, m: Messages, planned: boolean): string {
  const d = m.ops.stepDetail;
  switch (step.code) {
    case "fetch":
      return d.fetchOnly;
    case "pull": {
      const text = planned ? d.pullPlanned(step.commits, remoteLabel(step.from)) : d.pull(step.commits, remoteLabel(step.from));
      return step.prefer ? d.withPrefer(text, step.prefer) : text;
    }
    case "push":
      if (step.force) return d.forcePush(remoteLabel(step.to));
      if (step.createsRemoteBranch) return d.pushNew(remoteLabel(step.to));
      return d.push(step.commits, remoteLabel(step.to));
    case "commit":
      return d.commit(step.files);
    case "checkout":
      return d.checkout(step.branch);
    case "merge":
      return d.merge(step.source, step.target, step.commits);
    case "create_repo":
      return d.createRepo(step.nameWithOwner, step.visibility === "public" ? "Public" : "Private");
    case "add_remote":
      return d.addRemote(step.name);
    case "initial_commit":
      return d.initialCommit(step.files);
    case "ignore":
      return d.ignore(step.patterns);
    case "clone":
      return d.clone(step.destination);
    case "init":
      return m.ops.step.init;
  }
}

export function warningText(warning: PlanWarning, m: Messages): string {
  const w = m.ops.warning;
  switch (warning.code) {
    case "uncommitted_changes":
      return w.uncommitted_changes(warning.files);
    case "stale_remote":
      return w.stale_remote;
    case "creates_remote_branch":
      return w.creates_remote_branch(remoteLabel(warning.to));
    case "conflicts_likely":
      return w.conflicts_likely(warning.paths.length);
    case "conflicts_resolved":
      return w.conflicts_resolved(warning.paths.length, warning.prefer);
    case "skips_sensitive_files":
      return w.skips_sensitive_files(warning.paths.length);
    case "rewrites_published_history":
      return w.rewrites_published_history(warning.commits);
    case "public_repo":
      return w.public_repo;
    case "sensitive_files":
      return w.sensitive_files(warning.paths.length);
    case "no_gitignore":
      return w.no_gitignore;
    case "large_files":
      return w.large_files(warning.paths.length);
    case "many_files":
      return w.many_files(warning.files);
  }
}

export function resultText(kind: OperationKind, item: OperationItem, m: Messages): string {
  switch (item.state) {
    case "queued":
      return m.ops.state.queued;
    case "running":
      return item.step ? m.ops.stepRunning[item.step] : m.ops.running;
    case "succeeded": {
      const meaningful = item.completed.filter((step) => step.code !== "fetch" && step.code !== "checkout");
      if (meaningful.length === 0) return kind === "fetch" ? m.ops.state.succeeded : m.ops.stepDetail.upToDate;
      return meaningful.map((step) => stepDetail(step, m, false)).join(m.common.dot);
    }
    case "skipped":
      return m.ops.nothingToDo[kind];
    case "needs_confirmation":
      return item.reason ? m.ops.blocker[item.reason] : m.ops.state.needs_confirmation;
    case "conflict":
      return m.error.merge_conflict.hint;
    case "failed": {
      const text = item.error ? (m.error[item.error.code] ?? m.error.unknown) : m.error.unknown;
      const reason = text.hint ? `${text.title} — ${text.hint}` : text.title;
      const done = item.completed.filter((step) => step.code !== "fetch");
      if (done.length === 0) return reason;
      const kept = done.some((step) => step.code === "create_repo" || step.code === "add_remote");
      return m.common.sentences(reason, m.ops.failedPartial(done.map((step) => m.ops.step[step.code])), kept && m.ops.publishKept);
    }
    case "cancelled":
      return item.completed.length > 0
        ? m.ops.cancelledPartial(item.completed.map((step) => m.ops.step[step.code]))
        : m.ops.state.cancelled;
  }
}
