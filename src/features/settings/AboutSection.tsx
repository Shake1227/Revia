import clsx from "clsx";
import { useState } from "react";
import { CREATOR, LICENSE } from "../../app/meta";
import { Button } from "../../components/Button";
import { ErrorNote } from "../../components/ErrorNote";
import { Icons } from "../../components/icons";
import { LogoMark, Wordmark } from "../../components/Logo";
import { Switch } from "../../components/Switch";
import { useT } from "../../i18n";
import { api } from "../../ipc/client";
import { formatBytes } from "../../lib/format";
import { notifyError } from "../../lib/notify";
import { formatDay, relativeTime } from "../../lib/time";
import { useAppStore } from "../../stores/app";
import { useUpdateStore } from "../../stores/update";
import { Row, Section } from "../detail/SettingsTab";
import { useUiStore } from "../../stores/ui";
import styles from "./AboutSection.module.css";

const openLink = (url: string) => void api.openUrl({ url }).catch(notifyError);

function ReleaseNotes({ text }: { text: string }) {
  const lines = text.split(/\r?\n/).filter((line) => line.trim() !== "");
  return (
    <div className={clsx(styles.notes, "selectable")}>
      {lines.map((line, index) => {
        const heading = /^#{1,6}\s+(.*)$/.exec(line);
        if (heading) return <p key={index} className={styles.notesHeading}>{heading[1]}</p>;
        const item = /^\s*[-*]\s+(.*)$/.exec(line);
        if (item) return <p key={index} className={styles.notesItem}>{item[1]}</p>;
        return <p key={index}>{line}</p>;
      })}
    </div>
  );
}

function UpdateRows() {
  const m = useT();
  const t = m.settings.update;
  const info = useUpdateStore((state) => state.info);
  const phase = useUpdateStore((state) => state.phase);
  const progress = useUpdateStore((state) => state.progress);
  const download = useUpdateStore((state) => state.download);
  const error = useUpdateStore((state) => state.error);
  const autoCheck = useAppStore((state) => state.ui.updateAutoCheck);
  const checkedAt = useAppStore((state) => state.ui.updateCheckedAt);
  const setUi = useAppStore((state) => state.setUi);
  const [notesOpen, setNotesOpen] = useState(false);

  const store = useUpdateStore.getState();
  const latest = info?.latest ?? null;
  const available = info?.available === true && latest !== null;
  const asset = available ? latest.asset : null;
  const checking = phase === "checking";

  const label = !info
    ? t.title
    : available
      ? t.available(latest.version)
      : latest
        ? t.upToDate
        : t.noRelease;

  const facts: string[] = [];
  if (available) {
    if (latest.publishedAt !== null) facts.push(t.published(formatDay(latest.publishedAt, m)));
    if (asset) facts.push(formatBytes(asset.size, m));
  } else if (info) {
    facts.push(t.lastChecked(relativeTime(info.checkedAt, m)));
  } else if (checkedAt !== null) {
    facts.push(t.lastChecked(relativeTime(checkedAt, m)));
  }
  const hint = facts.length > 0 ? facts.join(m.common.dot) : info ? undefined : t.notChecked;

  const fraction = progress && progress.total ? Math.min(1, progress.received / progress.total) : null;

  return (
    <>
      <Row
        label={
          <span className={styles.status} data-available={available || undefined}>
            {available && <span className={styles.dot} />}
            {label}
          </span>
        }
        hint={hint}
      >
        {available && asset && phase === "idle" && (
          <Button size="sm" variant="primary" icon={<Icons.download size={13} />} onClick={() => void store.startDownload()}>
            {t.download}
          </Button>
        )}
        {available && !asset && (
          <Button size="sm" icon={<Icons.external size={13} />} onClick={() => openLink(latest.url)}>
            {t.openRelease}
          </Button>
        )}
        {(phase === "ready" || phase === "opened") && asset && (
          <Button size="sm" variant="primary" onClick={() => void store.install()}>
            {t.open[asset.kind]}
          </Button>
        )}
        {phase === "downloading" && (
          <Button size="sm" onClick={() => void store.cancelDownload()}>
            {t.cancel}
          </Button>
        )}
        {(phase === "idle" || checking) && (
          <Button size="sm" variant={available ? "ghost" : "secondary"} icon={<Icons.retry size={13} />} loading={checking} onClick={() => void store.check()}>
            {checking ? t.checking : t.check}
          </Button>
        )}
      </Row>

      {phase === "downloading" && (
        <div className={styles.progress} role="progressbar" aria-label={t.downloading} aria-valuemin={0} aria-valuemax={100} aria-valuenow={fraction === null ? undefined : Math.round(fraction * 100)}>
          <div className={styles.track}>
            <div className={styles.bar} data-indeterminate={fraction === null || undefined} style={fraction === null ? undefined : { transform: `scaleX(${fraction})` }} />
          </div>
          <p className={styles.progressText}>
            {t.downloading}
            {progress && (
              <span>
                {formatBytes(progress.received, m)}
                {progress.total ? ` / ${formatBytes(progress.total, m)}` : ""}
              </span>
            )}
          </p>
        </div>
      )}

      {(phase === "ready" || phase === "opened") && asset && download && (
        <div className={styles.ready}>
          <p className={styles.readyTitle}>
            <Icons.circleCheck size={14} />
            {t.ready}
          </p>
          <p>{t.guide[asset.kind]}</p>
          {!download.verified && <p className={styles.caution}>{t.unverified}</p>}
          <p className={clsx(styles.path, "mono selectable")}>{download.path}</p>
        </div>
      )}

      {available && !asset && <p className={styles.caution}>{t.noAsset}</p>}
      {error && <ErrorNote error={error} compact />}

      {available && latest.notes && (
        <div className={styles.notesBlock}>
          <button type="button" className={styles.notesToggle} aria-expanded={notesOpen} onClick={() => setNotesOpen((value) => !value)}>
            <Icons.chevronRight size={13} className={styles.chevron} data-open={notesOpen || undefined} />
            {t.notes}
          </button>
          {notesOpen && (
            <>
              <ReleaseNotes text={latest.notes} />
              <button type="button" className={styles.link} onClick={() => openLink(latest.url)}>
                {t.openRelease}
                <Icons.arrowUpRight size={12} />
              </button>
            </>
          )}
        </div>
      )}

      <Row label={t.autoCheck} hint={t.autoCheckHint}>
        <Switch checked={autoCheck} onChange={(updateAutoCheck) => setUi({ updateAutoCheck })} ariaLabel={t.autoCheck} />
      </Row>
    </>
  );
}

export function AboutSection() {
  const m = useT();
  const t = m.settings;
  const environment = useAppStore((state) => state.environment);

  return (
    <>
      <Section title={t.sections.update}>
        <UpdateRows />
      </Section>

      <Section title={t.sections.about}>
        <div className={styles.about}>
          <LogoMark size={44} weight="fine" />
          <div>
            <Wordmark />
            <p className={styles.tagline}>{m.app.tagline}</p>
            <p className={clsx(styles.versions, "selectable")}>
              <span>{t.about.version(environment?.appVersion ?? "")}</span>
              <span>
                {t.about.git} {environment?.git.installed ? (environment.git.version ?? "") : t.about.notFound}
              </span>
              <span>
                {t.about.gh} {environment?.gh.installed ? (environment.gh.version ?? "") : t.about.notFound}
              </span>
            </p>
          </div>
        </div>

        <Row label={CREATOR.name} hint={t.about.creator}>
          <Button size="sm" icon={<Icons.xMark size={11} />} onClick={() => openLink(CREATOR.x.url)}>
            {CREATOR.x.handle}
          </Button>
          <Button size="sm" icon={<Icons.github size={13} />} onClick={() => openLink(CREATOR.github.url)}>
            {CREATOR.github.handle}
          </Button>
        </Row>

        <Row label={`${t.about.license}${m.common.dot}${LICENSE.name}`} hint={t.about.licenseBody}>
          <Button size="sm" variant="ghost" icon={<Icons.file size={13} />} onClick={() => useUiStore.getState().openDialog({ type: "license" })}>
            {t.about.readLicense}
          </Button>
        </Row>
      </Section>
    </>
  );
}
