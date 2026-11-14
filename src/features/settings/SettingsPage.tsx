import clsx from "clsx";
import { useState } from "react";
import { Avatar } from "../../components/Avatar";
import { Button, IconButton } from "../../components/Button";
import { Icons } from "../../components/icons";
import { Kbd } from "../../components/Kbd";
import { Segmented } from "../../components/Segmented";
import { Select } from "../../components/Select";
import { Switch } from "../../components/Switch";
import { useT } from "../../i18n";
import { GITHUB_ACCESS_URL } from "../../app/meta";
import { api } from "../../ipc/client";
import type { GeneralSettingsPatch, ScanRoot } from "../../ipc/types";
import { notifyError } from "../../lib/notify";
import { displayPath } from "../../lib/path";
import { useAppStore, type Density, type MotionSetting, type ThemeSetting, type Typeface } from "../../stores/app";
import { confirm } from "../../stores/confirm";
import { useGithubStore } from "../../stores/github";
import { toast } from "../../stores/toasts";
import { useUiStore } from "../../stores/ui";
import { useAsync } from "../detail/hooks";
import { Row, Section } from "../detail/SettingsTab";
import { addFolder } from "../dialogs/AddDialog";
import { logoutGithub } from "../github/GithubPage";
import { PageHeader } from "../shell/PageHeader";
import { AboutSection } from "./AboutSection";
import { BackgroundSection } from "./BackgroundSection";
import styles from "./SettingsPage.module.css";

export function SettingsPage() {
  const m = useT();
  const t = m.settings;
  const ui = useAppStore((state) => state.ui);
  const setUi = useAppStore((state) => state.setUi);
  const settings = useAppStore((state) => state.settings);
  const updateGeneral = useAppStore((state) => state.updateGeneral);
  const applyWorkspace = useAppStore((state) => state.applyWorkspace);
  const github = useGithubStore((state) => state.status);
  const openDialog = useUiStore((state) => state.openDialog);
  const editors = useAsync(() => api.listEditors(), []);
  const [scanning, setScanning] = useState(false);
  if (!settings) return null;
  const general = settings.general;

  const patch = (value: GeneralSettingsPatch) => void updateGeneral(value).catch(notifyError);

  const workspaceAction = async (action: () => Promise<Parameters<typeof applyWorkspace>[0]>) => {
    try {
      applyWorkspace(await action());
    } catch (error) {
      notifyError(error);
    }
  };

  const removeRoot = async (root: ScanRoot) => {
    const result = await confirm({
      title: m.confirm.removeRoot.title,
      body: m.confirm.removeRoot.body(displayPath(root.path)),
      level: "safe",
      confirmLabel: m.confirm.removeRoot.submit,
    });
    if (result.confirmed) await workspaceAction(() => api.removeScanRoot({ rootId: root.id }));
  };

  const rescan = async () => {
    setScanning(true);
    try {
      const workspace = await api.scanProjects();
      applyWorkspace(workspace);
      toast.success(t.discovery.rescanned(workspace.projects.length));
    } catch (error) {
      notifyError(error);
    } finally {
      setScanning(false);
    }
  };

  const toggleSafety = async (key: "allowForcePush" | "allowHardReset", enabled: boolean) => {
    if (enabled) {
      const label = key === "allowForcePush" ? t.safety.forcePush : t.safety.hardReset;
      const hint = key === "allowForcePush" ? t.safety.forcePushHint : t.safety.hardResetHint;
      const result = await confirm({ title: label, risk: hint, level: "danger", confirmLabel: label, acknowledge: true });
      if (!result.confirmed) return;
    }
    patch({ safety: { [key]: enabled } });
  };

  const shortcuts: { group: string; items: [string, string[]][] }[] = [
    {
      group: t.shortcuts.groups.global,
      items: [
        [t.shortcuts.palette, ["mod", "K"]],
        [t.shortcuts.search, ["mod", "F"]],
        [t.shortcuts.refresh, ["mod", "R"]],
        [t.shortcuts.syncAll, ["mod", "shift", "S"]],
        [t.shortcuts.add, ["mod", "N"]],
        [t.shortcuts.navProjects, ["mod", "1"]],
        [t.shortcuts.navGithub, ["mod", "2"]],
        [t.shortcuts.navHistory, ["mod", "3"]],
        [t.shortcuts.settings, ["mod", ","]],
        [t.shortcuts.back, ["mod", "["]],
      ],
    },
    {
      group: t.shortcuts.groups.list,
      items: [
        [t.shortcuts.move, ["up", "down"]],
        [t.shortcuts.toggle, ["space"]],
        [t.shortcuts.open, ["enter"]],
        [t.shortcuts.selectAll, ["mod", "A"]],
        [t.shortcuts.clear, ["esc"]],
      ],
    },
  ];

  return (
    <div className={styles.page}>
      <PageHeader title={t.title} subtitle={t.subtitle} />
      <div className={`${styles.scroll} rv-sheet`}>
        <div className={styles.inner}>
          <Section title={t.sections.general}>
            <Row label={t.general.theme} hint={t.general.themeHint}>
              <Segmented<ThemeSetting>
                value={ui.theme}
                onChange={(theme) => setUi({ theme })}
                options={[
                  { value: "system", label: t.general.themes.system, icon: <Icons.monitor size={12} /> },
                  { value: "dark", label: t.general.themes.dark, icon: <Icons.moon size={12} /> },
                  { value: "light", label: t.general.themes.light, icon: <Icons.sun size={12} /> },
                  { value: "sky", label: t.general.themes.sky },
                ]}
              />
            </Row>
            <Row label={t.general.motion} hint={t.general.motionHint}>
              <Segmented<MotionSetting>
                value={ui.motion}
                onChange={(motion) => setUi({ motion })}
                options={(["system", "full", "reduced"] as const).map((value) => ({ value, label: t.general.motions[value] }))}
              />
            </Row>
            <Row label={t.general.typeface} hint={t.general.typefaceHint}>
              <Segmented<Typeface>
                value={ui.typeface}
                onChange={(typeface) => setUi({ typeface })}
                options={(["plex", "mplus", "system"] as const).map((value) => ({ value, label: t.general.typefaces[value] }))}
              />
            </Row>
            <Row label={t.general.density}>
              <Segmented<Density>
                value={ui.density}
                onChange={(density) => setUi({ density })}
                options={(["comfortable", "compact"] as const).map((value) => ({ value, label: t.general.densities[value] }))}
              />
            </Row>
            <Row label={t.general.language}>
              <Select
                size="sm"
                value={ui.language}
                onChange={(language) => setUi({ language })}
                options={[
                  { value: "ja", label: t.general.languages.ja },
                  { value: "en", label: t.general.languages.en },
                ]}
              />
            </Row>
            <Row label={t.general.editor} hint={t.general.editorHint}>
              <Select
                size="sm"
                value={general.editor ?? "__auto__"}
                onChange={(value) => patch({ editor: value === "__auto__" ? null : value })}
                options={[{ value: "__auto__", label: t.general.editorAuto }, ...(editors.data ?? []).map((editor) => ({ value: editor.id, label: editor.name }))]}
              />
            </Row>
          </Section>

          <BackgroundSection />

          <Section title={t.sections.discovery} hint={t.discovery.rootsHint}>
            {settings.roots.length > 0 && (
              <ul className={styles.roots}>
                {settings.roots.map((root) => (
                  <li key={root.id} className={styles.root} data-disabled={!root.enabled || undefined}>
                    <span className={styles.rootIcon}>{root.exists ? <Icons.folder size={15} /> : <Icons.folderX size={15} />}</span>
                    <div className={styles.rootMain}>
                      <span className={clsx("mono selectable truncate", styles.rootPath)}>{displayPath(root.path)}</span>
                      {!root.exists && <span className={styles.rootMissing}>{t.discovery.missing}</span>}
                    </div>
                    <Select
                      size="sm"
                      value={String(root.maxDepth)}
                      onChange={(value) => void workspaceAction(() => api.updateScanRoot({ rootId: root.id, patch: { maxDepth: Number(value) } }))}
                      options={[1, 2, 3, 4, 5, 6, 7, 8].map((value) => ({ value: String(value), label: t.discovery.depthValue(value) }))}
                      ariaLabel={t.discovery.depth}
                    />
                    <Switch
                      checked={root.enabled}
                      onChange={(enabled) => void workspaceAction(() => api.updateScanRoot({ rootId: root.id, patch: { enabled } }))}
                      ariaLabel={t.discovery.enabled}
                    />
                    <IconButton label={t.discovery.removeRoot} size="sm" onClick={() => void removeRoot(root)}>
                      <Icons.trash size={13} />
                    </IconButton>
                  </li>
                ))}
              </ul>
            )}
            <div className={styles.inline}>
              <Button size="sm" icon={<Icons.folderPlus size={13} />} onClick={() => void addFolder().catch(notifyError)}>
                {t.discovery.addRoot}
              </Button>
              <Button size="sm" variant="ghost" icon={<Icons.retry size={13} />} loading={scanning} disabled={settings.roots.length === 0} onClick={() => void rescan()}>
                {scanning ? t.discovery.rescanning : t.discovery.rescan}
              </Button>
            </div>
            <Row label={t.discovery.rules} hint={t.discovery.rulesBody} />
            <Row label={t.discovery.excluded} hint={settings.excludedPaths.length === 0 ? t.discovery.excludedEmpty : t.discovery.excludedHint} />
            {settings.excludedPaths.length > 0 && (
              <ul className={styles.roots}>
                {settings.excludedPaths.map((path) => (
                  <li key={path} className={styles.root}>
                    <span className={styles.rootIcon}>
                      <Icons.archive size={14} />
                    </span>
                    <span className={clsx("mono selectable truncate", styles.rootPath, styles.rootMain)}>{displayPath(path)}</span>
                    <Button size="sm" variant="ghost" onClick={() => void workspaceAction(() => api.restoreExcludedPath({ path }))}>
                      {t.discovery.restore}
                    </Button>
                  </li>
                ))}
              </ul>
            )}
          </Section>

          <Section title={t.sections.github}>
            {github?.authenticated && github.account ? (
              <>
                <Row
                  label={
                    <span className={styles.account}>
                      <Avatar name={github.account.login} src={github.account.avatarUrl} size={26} />
                      <span>
                        {github.account.name ?? github.account.login}
                        <span className={styles.accountSub}>@{github.account.login}</span>
                      </span>
                    </span>
                  }
                >
                  <Button size="sm" onClick={() => void logoutGithub()}>
                    {m.github.account.logout}
                  </Button>
                </Row>
                {(github.tokenStorage === "file" || github.tokenStorage === "environment") && (
                  <Row
                    label={m.github.account.storage}
                    hint={github.tokenStorage === "file" ? m.github.account.storageFile : m.github.account.storageEnvironment}
                  />
                )}
                <Row label={m.github.account.access} hint={m.github.account.accessHint}>
                  <Button
                    size="sm"
                    variant="ghost"
                    icon={<Icons.external size={13} />}
                    onClick={() => void api.openUrl({ url: GITHUB_ACCESS_URL }).catch(notifyError)}
                  >
                    {m.github.account.accessAction}
                  </Button>
                </Row>
              </>
            ) : (
              <Row label={m.github.connect.title} hint={m.github.connect.body}>
                <Button size="sm" variant="primary" icon={<Icons.github size={13} />} onClick={() => openDialog({ type: "login" })}>
                  {m.github.connect.action}
                </Button>
              </Row>
            )}
          </Section>

          <Section title={m.github.publish.title}>
            <Row label={m.github.publish.ignore.settingLabel} hint={m.github.publish.ignore.settingHint}>
              <Switch checked={ui.publishAutoIgnore} onChange={(publishAutoIgnore) => setUi({ publishAutoIgnore })} ariaLabel={m.github.publish.ignore.settingLabel} />
            </Row>
          </Section>

          <Section title={t.sections.sync}>
            <Row label={t.sync.onFocus} hint={t.sync.onFocusHint}>
              <Switch checked={general.autoRefresh.onFocus} onChange={(onFocus) => patch({ autoRefresh: { onFocus } })} ariaLabel={t.sync.onFocus} />
            </Row>
            <Row label={t.sync.interval}>
              <Select
                size="sm"
                value={String(general.autoRefresh.intervalMinutes)}
                onChange={(value) => patch({ autoRefresh: { intervalMinutes: Number(value) } })}
                options={[0, 5, 10, 30, 60].map((value) => ({ value: String(value), label: value === 0 ? t.sync.intervalOff : t.sync.intervalMinutes(value) }))}
              />
            </Row>
            <Row label={t.sync.autoFetch} hint={t.sync.autoFetchHint}>
              <Switch
                checked={general.autoRefresh.fetch}
                disabled={general.autoRefresh.intervalMinutes === 0}
                onChange={(fetch) => patch({ autoRefresh: { fetch } })}
                ariaLabel={t.sync.autoFetch}
              />
            </Row>
            <Row label={t.sync.concurrency} hint={t.sync.concurrencyHint}>
              <Select
                size="sm"
                value={String(general.concurrency)}
                onChange={(value) => patch({ concurrency: Number(value) })}
                options={[1, 2, 3, 4, 6, 8].map((value) => ({ value: String(value), label: String(value) }))}
              />
            </Row>
          </Section>

          <Section title={t.notifications.title}>
            <Row label={t.notifications.operationDone}>
              <Switch checked={ui.notifyOperationDone} onChange={(notifyOperationDone) => setUi({ notifyOperationDone })} ariaLabel={t.notifications.operationDone} />
            </Row>
            <Row label={t.notifications.errors}>
              <Switch checked={ui.notifyErrors} onChange={(notifyErrors) => setUi({ notifyErrors })} ariaLabel={t.notifications.errors} />
            </Row>
          </Section>

          <Section title={t.sections.safety} hint={t.safety.intro} tone="danger">
            <Row label={t.safety.forcePush} hint={t.safety.forcePushHint}>
              <Switch checked={general.safety.allowForcePush} onChange={(enabled) => void toggleSafety("allowForcePush", enabled)} ariaLabel={t.safety.forcePush} />
            </Row>
            <Row label={t.safety.hardReset} hint={t.safety.hardResetHint}>
              <Switch checked={general.safety.allowHardReset} onChange={(enabled) => void toggleSafety("allowHardReset", enabled)} ariaLabel={t.safety.hardReset} />
            </Row>
            <Row label={t.safety.always} hint={t.safety.alwaysBody} />
          </Section>

          <Section title={t.sections.shortcuts}>
            <div className={styles.shortcuts}>
              {shortcuts.map((group) => (
                <div key={group.group} className={styles.shortcutGroup}>
                  <p className={styles.shortcutTitle}>{group.group}</p>
                  {group.items.map(([label, keys]) => (
                    <div key={label} className={styles.shortcut}>
                      <span>{label}</span>
                      <Kbd keys={keys} />
                    </div>
                  ))}
                </div>
              ))}
            </div>
          </Section>

          <AboutSection />
        </div>
      </div>
    </div>
  );
}
