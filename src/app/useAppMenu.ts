import type { CheckMenuItemOptions, MenuItemOptions, PredefinedMenuItemOptions, SubmenuOptions } from "@tauri-apps/api/menu";
import { useEffect, useState } from "react";
import { addFolder, publishFolder } from "../features/dialogs/AddDialog";
import { TIME_PRESETS } from "../features/settings/BackgroundSection";
import { useT } from "../i18n";
import type { Messages } from "../i18n/ja";
import { api } from "../ipc/client";
import { notifyError } from "../lib/notify";
import { type BackgroundSettings, type SceneSetting, type ThemeSetting, useAppStore } from "../stores/app";
import { useProjectStore } from "../stores/projects";
import { type View, useUiStore } from "../stores/ui";
import { useUpdateStore } from "../stores/update";
import { CREATOR, LICENSE, REPOSITORY_URL } from "./meta";
import { fetchAllProjects, focusProjectSearch, overlayOpen, syncAllProjects, togglePalette } from "./useShortcuts";

type Entry = MenuItemOptions | CheckMenuItemOptions | PredefinedMenuItemOptions | SubmenuOptions;

const WINDOW_MENU = "revia-window";
const HELP_MENU = "revia-help";
const THEMES: ThemeSetting[] = ["system", "dark", "light", "sky"];
const SCENES: SceneSetting[] = ["lake", "hoshi", "plain"];

const separator: Entry = { item: "Separator" };

const whenIdle = (action: () => void) => () => {
  if (!overlayOpen()) action();
};

const go = (view: View) => whenIdle(() => useUiStore.getState().navigate(view));

const openUrl = (url: string) => () => void api.openUrl({ url }).catch(notifyError);

const setBackground = (patch: Partial<BackgroundSettings>) => {
  const state = useAppStore.getState();
  state.setUi({ background: { ...state.ui.background, ...patch } });
};

function build(m: Messages, redraw: () => void): SubmenuOptions[] {
  const t = m.menu;
  const { theme, background } = useAppStore.getState().ui;
  const choose = (action: () => void) => () => {
    action();
    redraw();
  };

  const scenes: SceneSetting[] = background.image !== null ? [...SCENES, "image"] : SCENES;

  return [
    {
      text: m.app.name,
      items: [
        { id: "about", text: t.about, action: go({ name: "settings" }) },
        {
          id: "check-update",
          text: t.checkUpdate,
          action: whenIdle(() => {
            useUiStore.getState().navigate({ name: "settings" });
            void useUpdateStore.getState().check();
          }),
        },
        separator,
        { id: "settings", text: t.settings, accelerator: "CmdOrCtrl+,", action: go({ name: "settings" }) },
        separator,
        { item: "Services", text: t.services },
        separator,
        { item: "Hide", text: t.hide },
        { item: "HideOthers", text: t.hideOthers },
        { item: "ShowAll", text: t.showAll },
        separator,
        { item: "Quit", text: t.quit },
      ],
    },
    {
      text: t.file,
      items: [
        {
          id: "add-project",
          text: t.addProject,
          accelerator: "CmdOrCtrl+N",
          action: whenIdle(() => useUiStore.getState().openDialog({ type: "add" })),
        },
        { id: "add-folder", text: t.addFolder, action: whenIdle(() => void addFolder().catch(notifyError)) },
        { id: "publish-folder", text: t.publishFolder, action: whenIdle(() => void publishFolder().catch(notifyError)) },
        separator,
        { item: "CloseWindow", text: t.closeWindow },
      ],
    },
    {
      text: t.edit,
      items: [
        { item: "Undo", text: t.undo },
        { item: "Redo", text: t.redo },
        separator,
        { item: "Cut", text: t.cut },
        { item: "Copy", text: t.copy },
        { item: "Paste", text: t.paste },
        { item: "SelectAll", text: t.selectAll },
        separator,
        { id: "find", text: t.find, accelerator: "CmdOrCtrl+F", action: whenIdle(focusProjectSearch) },
      ],
    },
    {
      text: t.project,
      items: [
        { id: "sync-all", text: t.syncAll, accelerator: "CmdOrCtrl+Shift+S", action: whenIdle(syncAllProjects) },
        { id: "fetch-all", text: t.fetchAll, action: whenIdle(fetchAllProjects) },
        separator,
        {
          id: "refresh",
          text: t.refresh,
          accelerator: "CmdOrCtrl+R",
          action: whenIdle(() => void useProjectStore.getState().refresh().catch(notifyError)),
        },
      ],
    },
    {
      text: t.view,
      items: [
        { id: "nav-projects", text: m.nav.projects, accelerator: "CmdOrCtrl+1", action: go({ name: "projects" }) },
        { id: "nav-github", text: m.nav.github, accelerator: "CmdOrCtrl+2", action: go({ name: "github" }) },
        { id: "nav-history", text: m.nav.history, accelerator: "CmdOrCtrl+3", action: go({ name: "history" }) },
        { id: "back", text: t.back, accelerator: "CmdOrCtrl+[", action: whenIdle(() => useUiStore.getState().back()) },
        separator,
        { id: "palette", text: t.palette, accelerator: "CmdOrCtrl+K", action: togglePalette },
        separator,
        {
          text: t.theme,
          items: THEMES.map((value) => ({
            id: `theme-${value}`,
            text: m.settings.general.themes[value],
            checked: theme === value,
            action: choose(() => useAppStore.getState().setUi({ theme: value })),
          })),
        },
        {
          text: t.scene,
          items: scenes.map((value) => ({
            id: `scene-${value}`,
            text: m.settings.background.scenes[value],
            checked: background.scene === value,
            action: choose(() => setBackground({ scene: value })),
          })),
        },
        {
          text: t.time,
          items: [
            {
              id: "time-auto",
              text: m.settings.background.timeAuto,
              checked: background.timeAuto,
              action: choose(() => setBackground({ timeAuto: true })),
            },
            separator,
            ...TIME_PRESETS.map(([name, hour]) => ({
              id: `time-${name}`,
              text: m.settings.background.presets[name],
              checked: !background.timeAuto && background.hour === hour,
              action: choose(() => setBackground({ timeAuto: false, hour })),
            })),
          ],
        },
        separator,
        { item: "Fullscreen", text: t.fullscreen },
      ],
    },
    {
      id: WINDOW_MENU,
      text: t.window,
      items: [
        { item: "Minimize", text: t.minimize },
        { item: "Maximize", text: t.zoom },
        separator,
        { item: "BringAllToFront", text: t.bringAllToFront },
      ],
    },
    {
      id: HELP_MENU,
      text: t.help,
      items: [
        { id: "repository", text: t.repository, action: openUrl(REPOSITORY_URL) },
        { id: "report-issue", text: t.reportIssue, action: openUrl(`${REPOSITORY_URL}/issues`) },
        separator,
        { id: "creator-x", text: t.creatorX(CREATOR.x.handle), action: openUrl(CREATOR.x.url) },
        { id: "creator-github", text: t.creatorGithub(CREATOR.github.handle), action: openUrl(CREATOR.github.url) },
        separator,
        {
          id: "license",
          text: t.license(LICENSE.name),
          action: whenIdle(() => useUiStore.getState().openDialog({ type: "license" })),
        },
      ],
    },
  ];
}

async function install(items: SubmenuOptions[]) {
  const { Menu, Submenu } = await import("@tauri-apps/api/menu");
  const menu = await Menu.new({ items });
  const [windowMenu, helpMenu] = await Promise.all([menu.get(WINDOW_MENU), menu.get(HELP_MENU)]);
  const previous = await menu.setAsAppMenu();
  if (windowMenu instanceof Submenu) await windowMenu.setAsWindowsMenuForNSApp();
  if (helpMenu instanceof Submenu) await helpMenu.setAsHelpMenuForNSApp();
  await previous?.close();
}

let queue: Promise<void> = Promise.resolve();

export function useAppMenu(enabled: boolean) {
  const m = useT();
  const theme = useAppStore((state) => state.ui.theme);
  const scene = useAppStore((state) => state.ui.background.scene);
  const timeAuto = useAppStore((state) => state.ui.background.timeAuto);
  const hour = useAppStore((state) => state.ui.background.hour);
  const hasImage = useAppStore((state) => state.ui.background.image !== null);
  const [revision, setRevision] = useState(0);

  useEffect(() => {
    if (!enabled) return;
    const timer = window.setTimeout(() => {
      const items = build(m, () => setRevision((value) => value + 1));
      queue = queue
        .then(() => install(items))
        .catch((error: unknown) => {
          notifyError(error);
        });
    }, 150);
    return () => window.clearTimeout(timer);
  }, [enabled, m, theme, scene, timeAuto, hour, hasImage, revision]);
}
