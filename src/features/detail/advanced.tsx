import { t } from "../../i18n";
import { api } from "../../ipc/client";
import { toAppError } from "../../ipc/errors";
import type { CommitInfo, Project, ResetMode } from "../../ipc/types";
import { notifyError } from "../../lib/notify";
import { useAppStore } from "../../stores/app";
import { confirm } from "../../stores/confirm";
import { runAndWait } from "../../stores/operations";
import { toast } from "../../stores/toasts";
import { useUiStore } from "../../stores/ui";
import { remoteLabel } from "../projects/model";

const commitList = (commits: CommitInfo[], total: number) => (
  <span className="selectable">
    {commits.slice(0, 5).map((commit) => (
      <span key={commit.sha} style={{ display: "block" }}>
        <span className="mono">{commit.shortSha}</span> {commit.subject}
      </span>
    ))}
    {total > 5 && <span style={{ display: "block", opacity: 0.7 }}>… +{total - 5}</span>}
  </span>
);

function handleFailure(error: unknown, projectId: string) {
  const appError = toAppError(error);
  if (appError.code === "merge_conflict") {
    toast.warning(t().error.merge_conflict.title, { description: t().error.merge_conflict.hint });
    useUiStore.getState().openProject(projectId, "changes");
    return;
  }
  notifyError(appError);
}

export async function cherryPickCommit(project: Project, commit: CommitInfo) {
  const m = t();
  const branch = project.status?.branch ?? "";
  const result = await confirm({
    title: m.confirm.cherryPick.title,
    body: m.confirm.cherryPick.body(commit.shortSha, branch),
    facts: [
      { label: m.confirm.project, value: project.name },
      { label: m.confirm.branch, value: <span className="mono">{branch}</span> },
      { label: m.confirm.impact, value: commit.subject },
    ],
    risk: m.confirm.cherryPick.risk,
    level: "caution",
    confirmLabel: m.confirm.cherryPick.submit,
  });
  if (!result.confirmed) return;
  try {
    await api.cherryPick({ projectId: project.id, shas: [commit.sha] });
    toast.success(m.confirm.cherryPick.done);
  } catch (error) {
    handleFailure(error, project.id);
  }
}

export async function resetToCommit(project: Project, commit: CommitInfo) {
  const m = t();
  const r = m.confirm.reset;
  const allowHard = useAppStore.getState().settings?.general.safety.allowHardReset ?? false;
  const choice = await confirm({
    title: r.title,
    body: `${commit.shortSha} ${commit.subject}`,
    facts: [
      { label: m.confirm.project, value: project.name },
      { label: m.confirm.branch, value: <span className="mono">{project.status?.branch}</span> },
    ],
    level: "caution",
    confirmLabel: r.mode.mixed,
    alternative: allowHard ? { label: r.mode.hard, value: "hard" } : undefined,
    risk: allowHard ? r.mode.mixedHint : m.common.sentences(r.mode.mixedHint, r.hardDisabled),
  });
  const mode: ResetMode | null = choice.confirmed ? "mixed" : choice.choice === "hard" ? "hard" : null;
  if (!mode) return;
  try {
    const preview = await api.previewReset({ projectId: project.id, target: commit.sha, mode });
    if (preview.blocker) {
      toast.warning(m.ops.blocker[preview.blocker], { description: project.name });
      return;
    }
    const hard = mode === "hard";
    const result = await confirm({
      title: r.title,
      body: hard ? r.mode.hardHint : r.mode.mixedHint,
      facts: [
        { label: m.confirm.project, value: project.name },
        { label: r.target, value: <span className="mono">{commit.shortSha}</span> },
        { label: m.confirm.impact, value: commitList(preview.commits, preview.commitCount) },
      ],
      risk: m.common.sentences(
        r.commits(preview.commitCount),
        preview.unpushedCount > 0 && r.unpushed(preview.unpushedCount),
        hard && preview.discardedFiles > 0 && r.discarded(preview.discardedFiles),
      ),
      level: hard || preview.unpushedCount > 0 ? "danger" : "caution",
      confirmLabel: r.submit,
      typeToConfirm: hard ? r.word : undefined,
      acknowledge: !hard && preview.unpushedCount > 0,
    });
    if (!result.confirmed) return;
    await api.reset({ projectId: project.id, target: commit.sha, mode, fingerprint: preview.fingerprint });
    toast.success(r.done);
  } catch (error) {
    notifyError(error);
  }
}

export async function rebaseOnto(project: Project, onto: string) {
  const m = t();
  const r = m.confirm.rebase;
  try {
    const preview = await api.previewRebase({ projectId: project.id, onto });
    if (preview.blocker) {
      toast.warning(preview.blocker === "dirty_worktree" ? r.dirty : m.ops.blocker[preview.blocker], { description: project.name });
      return;
    }
    const rewrites = preview.publishedCount > 0;
    const result = await confirm({
      title: r.title,
      body: r.body(preview.branch, onto),
      facts: [
        { label: m.confirm.project, value: project.name },
        { label: r.onto, value: <span className="mono">{onto}</span> },
        { label: m.confirm.impact, value: commitList(preview.commits, preview.commitCount) },
      ],
      risk: m.common.sentences(r.commits(preview.commitCount), rewrites && r.published(preview.publishedCount)),
      level: rewrites ? "danger" : "caution",
      confirmLabel: r.submit,
      acknowledge: rewrites,
    });
    if (!result.confirmed) return;
    await api.rebase({ projectId: project.id, onto, fingerprint: preview.fingerprint });
    toast.success(r.done);
  } catch (error) {
    handleFailure(error, project.id);
  }
}

export async function forcePush(project: Project) {
  const m = t();
  const f = m.confirm.forcePush;
  const allowed = useAppStore.getState().settings?.general.safety.allowForcePush ?? false;
  const target = project.status?.targets.push.target;
  if (!allowed) {
    toast.warning(m.error.feature_disabled.title, {
      description: f.disabled,
      action: { label: m.nav.settings, run: () => useUiStore.getState().navigate({ name: "settings" }) },
    });
    return;
  }
  if (!target) {
    useUiStore.getState().openDialog({ type: "target", projectId: project.id });
    return;
  }
  const request = { kind: "push" as const, items: [{ itemId: project.id, projectId: project.id, force: true, setUpstream: false }] };
  try {
    const plan = await api.planOperation({ request });
    const item = plan.items[0];
    if (!item?.executable) {
      toast.warning(item?.blocker ? m.ops.blocker[item.blocker] : m.ops.nothingRunnable, { description: project.name });
      return;
    }
    const lost = item.warnings.find((warning) => warning.code === "rewrites_published_history");
    const result = await confirm({
      title: f.title,
      body: f.body(remoteLabel(target)),
      facts: [
        { label: m.confirm.project, value: project.name },
        { label: m.confirm.branch, value: <span className="mono">{`${project.status?.branch} → ${remoteLabel(target)}`}</span> },
      ],
      risk: m.common.sentences(
        f.risk,
        lost?.code === "rewrites_published_history" && lost.commits > 0 && m.ops.warning.rewrites_published_history(lost.commits),
      ),
      level: "danger",
      confirmLabel: f.submit,
      typeToConfirm: f.word,
    });
    if (!result.confirmed) return;
    await runAndWait(request, plan);
  } catch (error) {
    notifyError(error);
  }
}
