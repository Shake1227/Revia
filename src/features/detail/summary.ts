import type { Messages } from "../../i18n";
import type { Project } from "../../ipc/types";
import { targetInvalid } from "../projects/model";

export function describeProject(project: Project, m: Messages): { headline: string; guidance: string } {
  const status = project.status;
  if (!status) return { headline: m.common.loading, guidance: "" };
  if (targetInvalid(project)) return { headline: m.headline.targetInvalid, guidance: m.guidance.targetInvalid };
  const g = m.guidance;
  const h = m.headline;
  switch (status.state) {
    case "synced":
      return { headline: h.synced, guidance: g.synced };
    case "dirty": {
      const extra =
        (status.ahead ?? 0) > 0 ? m.headline.ahead(status.ahead!) : (status.behind ?? 0) > 0 ? m.headline.behind(status.behind!) : "";
      return { headline: h.dirty(status.changes.total), guidance: extra ? m.common.sentences(g.dirty, extra) : g.dirty };
    }
    case "ahead":
      return { headline: h.ahead(status.ahead ?? 0), guidance: g.ahead };
    case "behind":
      return { headline: h.behind(status.behind ?? 0), guidance: g.behind };
    case "diverged":
      return { headline: h.diverged(status.ahead ?? 0, status.behind ?? 0), guidance: g.diverged };
    case "conflicted":
      return { headline: h.conflicted(status.changes.conflicted), guidance: g.conflicted };
    case "in_progress":
      return { headline: h.in_progress(status.inProgress ? m.inProgress[status.inProgress] : m.common.operation), guidance: g.in_progress };
    case "unpublished":
      return status.targets.push.target
        ? { headline: h.unpublished, guidance: g.unpublished }
        : { headline: h.unpublishedUnresolved, guidance: g.unpublishedUnresolved };
    default:
      return { headline: h[status.state], guidance: g[status.state] };
  }
}
