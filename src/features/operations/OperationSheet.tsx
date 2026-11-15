import clsx from "clsx";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useMemo, useState } from "react";
import { Button } from "../../components/Button";
import { Checkbox } from "../../components/Checkbox";
import { Dialog } from "../../components/Dialog";
import { ErrorNote } from "../../components/ErrorNote";
import { TextField } from "../../components/Field";
import { Icons } from "../../components/icons";
import { Spinner } from "../../components/Spinner";
import { StatusGlyph, itemStateVisual } from "../../components/StatusGlyph";
import { Tooltip } from "../../components/Tooltip";
import { spring } from "../../design/motion";
import { useT } from "../../i18n";
import type { OperationItem, OperationKind, OperationRequest, OperationSnapshot, PlanItem } from "../../ipc/types";
import { useAppStore } from "../../stores/app";
import { isTerminal, resolveBlocker, useOperationStore } from "../../stores/operations";
import { useProjectStore } from "../../stores/projects";
import { useUiStore } from "../../stores/ui";
import { resultText, stepChip, stepDetail, warningText } from "./describe";
import styles from "./OperationSheet.module.css";

const PLACEHOLDER_MESSAGE = "-";

function useItemName(projectId: string | null, itemId: string): string {
  const name = useProjectStore((state) => (projectId ? state.byId[projectId]?.name : undefined));
  const label = useOperationStore((state) => state.labels[itemId]);
  return name ?? label ?? itemId;
}

function PlanRow({ item, kind, onResolve }: { item: PlanItem; kind: OperationKind; onResolve: () => void }) {
  const m = useT();
  const name = useItemName(item.projectId, item.itemId);
  const project = useProjectStore((state) => (item.projectId ? state.byId[item.projectId] : undefined));
  const steps = item.steps;
  return (
    <li className={styles.row} data-disabled={!item.executable || undefined}>
      <StatusGlyph state={project?.status?.state ?? null} size="sm" />
      <div className={styles.rowMain}>
        <div className={styles.rowHead}>
          <span className={clsx(styles.name, "truncate")}>{name}</span>
          {project?.status?.branch && <span className={clsx(styles.branch, "mono truncate")}>{project.status.branch}</span>}
          {item.executable && item.risk !== "safe" && (
            <span className={styles.risk} data-level={item.risk}>
              {m.ops.risk[item.risk]}
            </span>
          )}
        </div>
        {item.executable ? (
          <div className={styles.flow}>
            {steps.map((step, index) => (
              <span key={`${step.code}-${index}`} className={styles.flowItem}>
                {index > 0 && <Icons.chevronRight size={11} className={styles.flowArrow} />}
                <Tooltip label={stepDetail(step, m, true)} side="top">
                  <span className={styles.chip} data-code={step.code} data-force={step.code === "push" && step.force ? true : undefined}>
                    {stepChip(step, m)}
                  </span>
                </Tooltip>
              </span>
            ))}
          </div>
        ) : (
          <p className={styles.blocker}>
            {item.blocker === "nothing_to_do" ? m.ops.nothingToDo[kind] : item.blocker ? m.ops.blocker[item.blocker] : ""}
          </p>
        )}
        {item.executable &&
          item.warnings
            .filter((warning) => !(warning.code === "stale_remote" && steps.some((step) => step.code === "fetch")))
            .map((warning, index) => (
            <p key={`${warning.code}-${index}`} className={styles.warning} data-severe={["sensitive_files", "rewrites_published_history", "conflicts_likely"].includes(warning.code) || undefined}>
              <Icons.circleAlert size={12} />
              <span>
                {warningText(warning, m)}
                {(warning.code === "sensitive_files" || warning.code === "conflicts_likely" || warning.code === "large_files") && (
                  <span className={clsx(styles.paths, "mono selectable")}> {warning.paths.slice(0, 4).join(", ")}{warning.paths.length > 4 ? " …" : ""}</span>
                )}
              </span>
            </p>
          ))}
      </div>
      {!item.executable && item.projectId && item.blocker && item.blocker !== "nothing_to_do" && (
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            onResolve();
            resolveBlocker(item.projectId!, item.blocker);
          }}
        >
          {m.ops.confirmInProject}
        </Button>
      )}
    </li>
  );
}

function RunRow({ item, kind, onClose }: { item: OperationItem; kind: OperationKind; onClose: () => void }) {
  const m = useT();
  const name = useItemName(item.projectId, item.itemId);
  const [showDetail, setShowDetail] = useState(false);
  const running = item.state === "running";
  const visual = itemStateVisual[item.state];
  const Icon = visual.icon;
  const openProject = useUiStore((state) => state.openProject);

  return (
    <motion.li layout="position" className={styles.row} data-state={item.state} transition={spring.smooth}>
      <span className={styles.stateGlyph} data-tone={visual.tone} data-state={item.state}>
        <AnimatePresence mode="popLayout" initial={false}>
          <motion.span
            key={item.state}
            initial={{ scale: 0.4, opacity: 0 }}
            animate={{ scale: 1, opacity: 1, transition: item.state === "succeeded" ? spring.lively : spring.snappy }}
            exit={{ scale: 0.5, opacity: 0, transition: { duration: 0.1 } }}
            className={styles.stateInner}
          >
            {running ? <Spinner size={15} progress={item.progress} /> : <Icon size={12} strokeWidth={2.2} />}
          </motion.span>
        </AnimatePresence>
      </span>
      <div className={styles.rowMain}>
        <div className={styles.rowHead}>
          <span className={clsx(styles.name, "truncate")}>{name}</span>
          <span className={styles.stateLabel} data-tone={visual.tone}>
            {m.ops.state[item.state]}
          </span>
        </div>
        <p className={clsx(styles.result, running && styles.resultRunning)}>{resultText(kind, item, m)}</p>
        {item.error?.detail && showDetail && <pre className={clsx(styles.detail, "mono selectable")}>{item.error.detail}</pre>}
      </div>
      <div className={styles.rowActions}>
        {item.error?.detail && (
          <Button size="sm" variant="ghost" onClick={() => setShowDetail((value) => !value)}>
            {showDetail ? m.common.hideDetails : m.common.details}
          </Button>
        )}
        {item.state === "needs_confirmation" && item.projectId && (
          <Button
            size="sm"
            onClick={() => {
              onClose();
              resolveBlocker(item.projectId!, item.reason);
            }}
          >
            {m.ops.confirmInProject}
          </Button>
        )}
        {item.state === "conflict" && item.projectId && (
          <Button
            size="sm"
            onClick={() => {
              onClose();
              openProject(item.projectId!, "changes");
            }}
          >
            {m.action.resolve}
          </Button>
        )}
      </div>
    </motion.li>
  );
}

const PROGRESS_PARTS = ["succeeded", "needs_confirmation", "conflict", "failed", "stopped", "running", "queued"] as const;

type ProgressPart = (typeof PROGRESS_PARTS)[number];

const progressPart = (state: OperationSnapshot["items"][number]["state"]): ProgressPart =>
  state === "skipped" || state === "cancelled" ? "stopped" : state;

function ProgressBar({ snapshot }: { snapshot: OperationSnapshot }) {
  const counts = new Map<ProgressPart, number>();
  for (const item of snapshot.items) {
    const part = progressPart(item.state);
    counts.set(part, (counts.get(part) ?? 0) + 1);
  }
  return (
    <div className={styles.progress} aria-hidden>
      {PROGRESS_PARTS.map((part) => (
        <span key={part} className={styles.part} data-part={part} style={{ flexGrow: counts.get(part) ?? 0 }} />
      ))}
    </div>
  );
}

export function OperationSheet() {
  const m = useT();
  const sheet = useOperationStore((state) => state.sheet);
  const operations = useOperationStore((state) => state.operations);
  const execute = useOperationStore((state) => state.execute);
  const cancel = useOperationStore((state) => state.cancel);
  const closeSheet = useOperationStore((state) => state.closeSheet);
  const sync = useAppStore((state) => state.settings?.general.sync);
  const [message, setMessage] = useState("");
  const [acknowledged, setAcknowledged] = useState(false);
  const [shown, setShown] = useState(sheet);

  useEffect(() => {
    if (sheet) setShown(sheet);
  }, [sheet]);
  useEffect(() => {
    if (sheet?.phase === "plan" && sheet.plan === null) {
      setMessage("");
      setAcknowledged(false);
    }
  }, [sheet]);

  const current = sheet ?? shown;
  const snapshot = current?.phase === "run" ? operations[current.operationId] : undefined;
  const kind: OperationKind | undefined = current?.phase === "plan" ? current.request.kind : snapshot?.kind;

  const planGroups = useMemo(() => {
    if (current?.phase !== "plan" || !current.plan) return null;
    return {
      runnable: current.plan.items.filter((item) => item.executable),
      blocked: current.plan.items.filter((item) => !item.executable),
    };
  }, [current]);

  if (!current || !kind) return null;

  const open = sheet !== null;

  if (current.phase === "plan") {
    const total = current.request.items.length;
    const runnable = planGroups?.runnable ?? [];
    const blocked = planGroups?.blocked ?? [];
    const hasDanger = runnable.some((item) => item.risk === "danger");
    const needsMessage = kind === "commit";
    const disabled =
      !planGroups || runnable.length === 0 || current.starting || (needsMessage && message.trim() === "") || (hasDanger && !acknowledged);
    const transform = needsMessage
      ? (request: OperationRequest): OperationRequest =>
          request.kind === "commit" ? { kind: "commit", items: request.items.map((item) => ({ ...item, message: message.trim() })) } : request
      : undefined;

    return (
      <Dialog
        open={open}
        onOpenChange={(next) => !next && closeSheet()}
        title={m.ops.title[kind](total)}
        description={kind === "sync" && sync ? m.ops.syncNote(sync.commit, sync.diverged) : kind === "commit" ? m.ops.commitAllNote : undefined}
        size="lg"
        tone={hasDanger ? "danger" : "default"}
        flush
        footerStart={
          planGroups ? (
            <span className="tabular">
              {m.ops.willRun(runnable.length)}
              {blocked.length > 0 && `${m.common.dot}${m.ops.wontRun(blocked.length)}`}
            </span>
          ) : undefined
        }
        footer={
          <>
            <Button variant="ghost" onClick={closeSheet}>
              {m.common.cancel}
            </Button>
            <Button variant={hasDanger ? "dangerSolid" : "primary"} disabled={disabled} loading={current.starting} onClick={() => void execute(transform)} autoFocus={!needsMessage}>
              {m.ops.run[kind](runnable.length)}
            </Button>
          </>
        }
      >
        <div className={styles.body}>
          {needsMessage && (
            <div className={styles.inset}>
              <TextField
                value={message}
                onChange={(event) => setMessage(event.target.value)}
                placeholder={m.ops.commitMessagePlaceholder}
                aria-label={m.ops.commitMessage}
                autoFocus
                fieldSize="lg"
                onKeyDown={(event) => {
                  if (event.key === "Enter" && !disabled) void execute(transform);
                }}
              />
            </div>
          )}
          {current.error && (
            <div className={styles.inset}>
              <ErrorNote error={current.error} />
            </div>
          )}
          {!planGroups && !current.error && (
            <div className={styles.planning}>
              <Spinner size={16} />
              <span>{m.ops.planning}</span>
            </div>
          )}
          {planGroups && (
            <>
              {runnable.length === 0 && <p className={styles.nothing}>{m.ops.nothingRunnable}</p>}
              {runnable.length > 0 && (
                <ul className={styles.list}>
                  {runnable.map((item) => (
                    <PlanRow key={item.itemId} item={item} kind={kind} onResolve={closeSheet} />
                  ))}
                </ul>
              )}
              {blocked.length > 0 && (
                <>
                  <p className={styles.groupLabel}>
                    {m.ops.wontRun(blocked.length)}
                    <span>{m.ops.wontRunHint}</span>
                  </p>
                  <ul className={styles.list}>
                    {blocked.map((item) => (
                      <PlanRow key={item.itemId} item={item} kind={kind} onResolve={closeSheet} />
                    ))}
                  </ul>
                </>
              )}
              {hasDanger && (
                <div className={styles.inset}>
                  <Checkbox checked={acknowledged} onChange={setAcknowledged} label={m.confirm.acknowledge} />
                </div>
              )}
            </>
          )}
        </div>
      </Dialog>
    );
  }

  if (!snapshot) return null;
  const total = snapshot.items.length;
  const done = snapshot.items.filter((item) => isTerminal(item.state)).length;
  const succeeded = snapshot.items.filter((item) => item.state === "succeeded" || item.state === "skipped").length;
  const issues = snapshot.items.filter((item) => ["failed", "needs_confirmation", "conflict"].includes(item.state)).length;
  const finished = snapshot.state === "finished";
  const wasCancelled = finished && snapshot.items.some((item) => item.state === "cancelled");

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => !next && closeSheet()}
      title={m.ops.title[snapshot.kind](total)}
      description={
        finished
          ? wasCancelled
            ? m.ops.cancelled
            : issues > 0
              ? m.ops.finishedWithIssues(issues)
              : m.ops.finished
          : snapshot.state === "cancelling"
            ? m.ops.cancelling
            : m.ops.progress(done, total)
      }
      size="lg"
      flush
      footerStart={finished ? <span className="tabular">{m.ops.resultSummary(succeeded, total)}</span> : undefined}
      footer={
        finished ? (
          <Button variant="primary" onClick={closeSheet} autoFocus>
            {m.common.close}
          </Button>
        ) : (
          <>
            <Button variant="ghost" onClick={closeSheet}>
              {m.ops.hide}
            </Button>
            <Button variant="danger" disabled={snapshot.state === "cancelling"} onClick={() => void cancel(snapshot.operationId)}>
              {m.ops.cancel}
            </Button>
          </>
        )
      }
    >
      <div className={styles.body}>
        <div className={styles.inset}>
          <ProgressBar snapshot={snapshot} />
        </div>
        <ul className={styles.list}>
          {snapshot.items.map((item) => (
            <RunRow key={item.itemId} item={item} kind={snapshot.kind} onClose={closeSheet} />
          ))}
        </ul>
      </div>
    </Dialog>
  );
}
