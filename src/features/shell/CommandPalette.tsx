import { Command } from "cmdk";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState, type ReactNode } from "react";
import { Icons } from "../../components/icons";
import { Kbd } from "../../components/Kbd";
import { ToneDot, stateVisual } from "../../components/StatusGlyph";
import { dialogVariants, overlayVariants } from "../../design/motion";
import { useT } from "../../i18n";
import { notifyError } from "../../lib/notify";
import { displayPath } from "../../lib/path";
import { useAppStore } from "../../stores/app";
import { useProjectStore } from "../../stores/projects";
import { useUiStore } from "../../stores/ui";
import { fetchAllProjects, syncAllProjects } from "../../app/useShortcuts";
import { addFolder, publishFolder } from "../dialogs/AddDialog";
import { useAllProjects } from "../projects/useVisibleProjects";
import { useUpdateStore } from "../../stores/update";
import styles from "./CommandPalette.module.css";

export function CommandPalette() {
  const m = useT();
  const t = m.palette;
  const open = useUiStore((state) => state.paletteOpen);
  const setOpen = useUiStore((state) => state.setPaletteOpen);
  const navigate = useUiStore((state) => state.navigate);
  const openProject = useUiStore((state) => state.openProject);
  const openDialog = useUiStore((state) => state.openDialog);
  const setUi = useAppStore((state) => state.setUi);
  const projects = useAllProjects();
  const [query, setQuery] = useState("");

  useEffect(() => {
    if (open) setQuery("");
  }, [open]);

  const run = (action: () => void) => () => {
    setOpen(false);
    action();
  };

  const item = (key: string, icon: ReactNode, label: string, action: () => void, extra?: { keywords?: string[]; shortcut?: string[]; hint?: string }) => (
    <Command.Item key={key} value={`${label} ${key}`} keywords={extra?.keywords} onSelect={run(action)} className={styles.item}>
      <span className={styles.icon}>{icon}</span>
      <span className={styles.label}>{label}</span>
      {extra?.hint && <span className={`${styles.hint} truncate`}>{extra.hint}</span>}
      {extra?.shortcut && <Kbd keys={extra.shortcut} />}
    </Command.Item>
  );

  return (
    <AnimatePresence>
      {open && (
        <>
          <motion.div className={styles.overlay} variants={overlayVariants} initial="initial" animate="animate" exit="exit" onClick={() => setOpen(false)} />
          <div className={styles.positioner}>
            <motion.div className={styles.palette} variants={dialogVariants} initial="initial" animate="animate" exit="exit">
              <Command
                label={m.nav.palette}
                loop
                onKeyDown={(event) => {
                  if (event.key === "Escape") {
                    event.preventDefault();
                    event.stopPropagation();
                    setOpen(false);
                  }
                }}
              >
                <div className={styles.inputRow}>
                  <Icons.search size={16} />
                  <Command.Input autoFocus value={query} onValueChange={setQuery} placeholder={t.placeholder} className={styles.input} />
                  <Kbd keys={["esc"]} />
                </div>
                <Command.List className={styles.list}>
                  <Command.Empty className={styles.empty}>{t.empty}</Command.Empty>
                  <Command.Group heading={t.groups.actions} className={styles.group}>
                    {item("sync-all", <Icons.refresh size={15} />, t.syncAll, syncAllProjects, { shortcut: ["mod", "shift", "S"], keywords: ["sync", "同期"] })}
                    {item("fetch-all", <Icons.download size={15} />, t.fetchAll, fetchAllProjects, { keywords: ["fetch"] })}
                    {item("refresh", <Icons.retry size={15} />, t.refreshAll, () => void useProjectStore.getState().refresh().catch(notifyError), { shortcut: ["mod", "R"], keywords: ["refresh", "更新"] })}
                    {item("add-folder", <Icons.folderPlus size={15} />, t.addFolder, () => void addFolder().catch(notifyError), { keywords: ["add", "追加"] })}
                    {item("add", <Icons.plus size={15} />, m.add.title, () => openDialog({ type: "add" }), { shortcut: ["mod", "N"] })}
                    {item("publish-folder", <Icons.cloudUpload size={15} />, m.add.publishTitle, () => void publishFolder().catch(notifyError), { keywords: ["publish", "github", "git", "公開", "登録"] })}
                    {item(
                      "check-update",
                      <Icons.arrowUpRight size={15} />,
                      m.settings.update.command,
                      () => {
                        navigate({ name: "settings" });
                        void useUpdateStore.getState().check();
                      },
                      { keywords: ["update", "version", "アップデート", "更新"] },
                    )}
                  </Command.Group>
                  <Command.Group heading={t.groups.navigate} className={styles.group}>
                    {item("nav-projects", <Icons.layers size={15} />, m.nav.projects, () => navigate({ name: "projects" }), { shortcut: ["mod", "1"] })}
                    {item("nav-github", <Icons.github size={15} />, m.nav.github, () => navigate({ name: "github" }), { shortcut: ["mod", "2"] })}
                    {item("nav-history", <Icons.history size={15} />, m.nav.history, () => navigate({ name: "history" }), { shortcut: ["mod", "3"] })}
                    {item("nav-settings", <Icons.settings size={15} />, m.nav.settings, () => navigate({ name: "settings" }), { shortcut: ["mod", ","] })}
                  </Command.Group>
                  <Command.Group heading={t.groups.projects} className={styles.group}>
                    {projects.map((project) =>
                      item(
                        `project-${project.id}`,
                        <ToneDot tone={project.status ? stateVisual[project.status.state].tone : "neutral"} />,
                        project.name,
                        () => openProject(project.id),
                        { hint: displayPath(project.path), keywords: [project.path, project.status?.branch ?? ""] },
                      ),
                    )}
                  </Command.Group>
                  <Command.Group heading={t.groups.appearance} className={styles.group}>
                    {item("theme-dark", <Icons.moon size={15} />, t.themeDark, () => setUi({ theme: "dark" }), { keywords: ["theme", "dark"] })}
                    {item("theme-light", <Icons.sun size={15} />, t.themeLight, () => setUi({ theme: "light" }), { keywords: ["theme", "light"] })}
                    {item("theme-system", <Icons.monitor size={15} />, t.themeSystem, () => setUi({ theme: "system" }), { keywords: ["theme", "system"] })}
                  </Command.Group>
                </Command.List>
                <div className={styles.footer}>
                  <span>
                    <Kbd keys={["up", "down"]} /> {t.hint.move}
                  </span>
                  <span>
                    <Kbd keys={["enter"]} /> {t.hint.select}
                  </span>
                </div>
              </Command>
            </motion.div>
          </div>
        </>
      )}
    </AnimatePresence>
  );
}
