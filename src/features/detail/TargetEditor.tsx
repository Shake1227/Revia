import { useState } from "react";
import { Button } from "../../components/Button";
import { Note } from "../../components/ErrorNote";
import { Select, type SelectOption } from "../../components/Select";
import { useT } from "../../i18n";
import { api } from "../../ipc/client";
import type { BranchInfo, Project, RemoteRef, TargetState } from "../../ipc/types";
import { notifyError } from "../../lib/notify";
import { useProjectStore } from "../../stores/projects";
import { remoteLabel } from "../projects/model";
import styles from "./TargetEditor.module.css";

interface TargetEditorProps {
  project: Project;
  branches: BranchInfo[];
}

const AUTO = "__auto__";
const encode = (ref: RemoteRef) => `${ref.remote}\n${ref.branch}`;
const decode = (value: string): RemoteRef => {
  const [remote, ...rest] = value.split("\n");
  return { remote: remote!, branch: rest.join("\n") };
};

export function TargetEditor({ project, branches }: TargetEditorProps) {
  const m = useT();
  const t = m.settingsProject;
  const status = project.status;
  const [saving, setSaving] = useState<"push" | "pull" | null>(null);
  if (!status?.branch) return <Note tone="info">{t.targetsNoBranch}</Note>;
  const branch = status.branch;
  const remoteBranches = branches.filter((item) => item.kind === "remote" && item.remote);

  const save = async (kind: "push" | "pull", value: string) => {
    setSaving(kind);
    try {
      const updated = await api.setBranchTarget({ projectId: project.id, branch, kind, target: value === AUTO ? null : decode(value) });
      useProjectStore.getState().upsert(updated);
    } catch (error) {
      notifyError(error);
    } finally {
      setSaving(null);
    }
  };

  const optionsFor = (kind: "push" | "pull", state: TargetState): SelectOption<string>[] => {
    const auto = state.saved ? null : state.target;
    const options: SelectOption<string>[] = [
      { value: AUTO, label: auto ? `${t.auto}（${remoteLabel(auto)}）` : t.auto, hint: auto ? m.detail.target.source[auto.source] : undefined },
    ];
    for (const item of remoteBranches) {
      options.push({ value: encode({ remote: item.remote!, branch: item.shortName }), label: item.name, group: item.remote! });
    }
    if (kind === "push") {
      for (const remote of status.remotes) {
        if (!remoteBranches.some((item) => item.remote === remote.name && item.shortName === branch)) {
          options.push({ value: encode({ remote: remote.name, branch }), label: t.newRemoteBranch(`${remote.name}/${branch}`), group: remote.name });
        }
      }
    }
    if (state.saved && !options.some((option) => option.value === encode(state.saved!))) {
      options.push({ value: encode(state.saved), label: remoteLabel(state.saved), disabled: true });
    }
    return options;
  };

  const row = (kind: "push" | "pull", label: string, state: TargetState) => (
    <div className={styles.row} key={kind}>
      <div className={styles.label}>
        <span>{label}</span>
        {state.savedInvalid ? (
          <span className={styles.invalid}>{m.detail.target.invalid[state.savedInvalid]}</span>
        ) : state.saved ? (
          <span className={styles.saved}>{m.detail.target.source.saved}</span>
        ) : null}
      </div>
      <Select
        mono
        className={styles.select}
        value={state.saved ? encode(state.saved) : AUTO}
        onChange={(value) => void save(kind, value)}
        options={optionsFor(kind, state)}
        disabled={saving !== null}
        ariaLabel={label}
      />
      <Button size="sm" variant="ghost" disabled={!state.saved || saving !== null} onClick={() => void save(kind, AUTO)}>
        {t.reset}
      </Button>
    </div>
  );

  return (
    <div className={styles.editor}>
      {row("push", m.detail.target.push, status.targets.push)}
      {row("pull", m.detail.target.pull, status.targets.pull)}
    </div>
  );
}
