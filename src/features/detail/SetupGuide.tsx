import { useState } from "react";
import { Button } from "../../components/Button";
import { Icons } from "../../components/icons";
import { useT } from "../../i18n";
import { api } from "../../ipc/client";
import type { Project } from "../../ipc/types";
import { notifyError } from "../../lib/notify";
import { useProjectStore } from "../../stores/projects";
import { useUiStore } from "../../stores/ui";
import styles from "./SetupGuide.module.css";

export function SetupGuide({ project }: { project: Project }) {
  const m = useT();
  const t = m.detail.setup;
  const openDialog = useUiStore((state) => state.openDialog);
  const [pending, setPending] = useState(false);
  const status = project.status;
  if (!status) return null;

  const initialized = status.state !== "not_initialized";
  const committed = initialized && !status.unborn;
  const published = status.remotes.length > 0;
  const current = !initialized ? 0 : !committed ? 1 : 2;

  const init = async () => {
    setPending(true);
    try {
      useProjectStore.getState().upsert(await api.initRepository({ projectId: project.id, initialBranch: null }));
    } catch (error) {
      notifyError(error);
    } finally {
      setPending(false);
    }
  };

  const steps = [
    {
      title: t.steps.init,
      body: t.initBody,
      done: initialized,
      action: (
        <Button variant="primary" size="sm" loading={pending} onClick={() => void init()}>
          {t.initAction}
        </Button>
      ),
    },
    { title: t.steps.commit, body: t.commitBody, done: committed, action: null },
    {
      title: t.steps.publish,
      body: t.publishBody,
      done: published,
      action: (
        <>
          <Button variant="primary" size="sm" icon={<Icons.github size={13} />} onClick={() => openDialog({ type: "publish", projectIds: [project.id] })}>
            {t.publishAction}
          </Button>
          <Button size="sm" variant="ghost" onClick={() => openDialog({ type: "link", projectId: project.id })}>
            {t.linkAction}
          </Button>
        </>
      ),
    },
  ];

  return (
    <section className={styles.guide}>
      <h2 className={styles.title}>{t.title}</h2>
      <ol className={styles.steps}>
        {steps.map((step, index) => {
          const state = step.done ? "done" : index === current ? "current" : "upcoming";
          return (
            <li key={step.title} className={styles.step} data-state={state}>
              <span className={styles.marker}>{step.done ? <Icons.check size={12} strokeWidth={2.4} /> : index + 1}</span>
              <div className={styles.content}>
                <p className={styles.stepTitle}>{step.title}</p>
                {state === "current" && (
                  <>
                    <p className={styles.body}>{step.body}</p>
                    {step.action && <div className={styles.actions}>{step.action}</div>}
                  </>
                )}
              </div>
            </li>
          );
        })}
      </ol>
    </section>
  );
}
