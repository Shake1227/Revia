import { AnimatePresence, MotionConfig, motion } from "motion/react";
import { useEffect } from "react";
import { Backdrop, type Scene } from "../components/Backdrop";
import { skyTone } from "../components/lake";
import { Button } from "../components/Button";
import { ConfirmHost } from "../components/ConfirmHost";
import { ErrorNote } from "../components/ErrorNote";
import { LogoMark } from "../components/Logo";
import { Toaster } from "../components/Toaster";
import { TooltipProvider } from "../components/Tooltip";
import { pageVariants, spring } from "../design/motion";
import { ProjectDetailPage } from "../features/detail/ProjectDetailPage";
import { GlobalDialogs } from "../features/dialogs/GlobalDialogs";
import { GithubPage } from "../features/github/GithubPage";
import { HistoryPage } from "../features/history/HistoryPage";
import { OperationSheet } from "../features/operations/OperationSheet";
import { ProjectsPage } from "../features/projects/ProjectsPage";
import { SettingsPage } from "../features/settings/SettingsPage";
import { CommandPalette } from "../features/shell/CommandPalette";
import { Notices } from "../features/shell/Notices";
import { Dock } from "../features/shell/Dock";
import { TopBar } from "../features/shell/TopBar";
import { WindowControls } from "../features/shell/WindowControls";
import { useT } from "../i18n";
import { isTauri } from "../ipc/client";
import { ownsWindowFrame } from "../ipc/window";
import { useAppStore } from "../stores/app";
import { useBackgroundStore } from "../stores/background";
import { useGithubStore } from "../stores/github";
import { useUiStore } from "../stores/ui";
import { useUpdateStore } from "../stores/update";
import styles from "./App.module.css";
import { useAppMenu } from "./useAppMenu";
import { useAppearance } from "./useAppearance";
import { useAutoRefresh } from "./useAutoRefresh";
import { useBackendEvents } from "./useBackendEvents";
import { useShortcuts } from "./useShortcuts";
import { useSkyHour } from "./useSkyHour";

export function App() {
  const m = useT();
  const phase = useAppStore((state) => state.phase);
  const bootError = useAppStore((state) => state.bootError);
  const background = useAppStore((state) => state.ui.background);
  const platform = useAppStore((state) => state.environment?.platform);
  const imageUrl = useBackgroundStore((state) => state.url);
  const view = useUiStore((state) => state.view);
  const direction = useUiStore((state) => state.direction);
  const celebrations = useUiStore((state) => state.celebrations);
  const hour = useSkyHour(background.timeAuto, background.hour);
  const scene: Scene = background.scene === "image" ? (imageUrl !== null ? "image" : "lake") : background.scene;
  const tone = scene === "lake" ? skyTone(hour) : scene === "hoshi" ? "dark" : null;
  const { reducedMotion } = useAppearance(tone);
  const ready = phase === "ready";

  useEffect(() => {
    void useAppStore.getState().boot();
  }, []);

  useEffect(() => {
    if (!ready) return;
    void useGithubStore.getState().loadStatus();
    void useUpdateStore.getState().autoCheck();
  }, [ready]);

  useEffect(() => {
    if (ready) void useBackgroundStore.getState().sync(background.image);
  }, [ready, background.image]);

  useEffect(() => {
    document.documentElement.style.setProperty("--panel-alpha", String(background.panel / 100));
  }, [background.panel]);

  useEffect(() => {
    if (platform) document.documentElement.dataset.platform = platform;
  }, [platform]);

  const macWindow = isTauri && (platform ?? "macos") === "macos";

  useBackendEvents(ready);
  useAppMenu(ready && macWindow);
  useAutoRefresh(ready);
  useShortcuts();

  const viewKey = view.name === "project" ? `project:${view.projectId}` : view.name;

  return (
    <MotionConfig reducedMotion={reducedMotion ? "always" : "never"} transition={spring.smooth}>
      <TooltipProvider delayDuration={420} skipDelayDuration={220}>
        <div className={styles.app}>
          <Backdrop
            scene={scene}
            hour={hour}
            live={background.timeAuto}
            imageUrl={imageUrl}
            frame={background.frame}
            dim={background.dim / 100}
            blur={background.blur}
            animated={background.animate && !reducedMotion}
            celebrate={celebrations}
          />

          <AnimatePresence>
            {phase === "loading" && (
              <motion.div
                key="splash"
                className={styles.splash}
                data-tauri-drag-region
                initial={{ opacity: 1 }}
                exit={{ opacity: 0, scale: 1.02, transition: { duration: 0.3 } }}
              >
                <motion.div
                  initial={{ opacity: 0, scale: 0.9 }}
                  animate={{ opacity: 1, scale: 1 }}
                  transition={{ ...spring.gentle, delay: 0.05 }}
                >
                  <LogoMark size={56} weight="fine" drawn />
                </motion.div>
              </motion.div>
            )}
          </AnimatePresence>

          {phase === "failed" && bootError && (
            <div className={styles.failed} data-tauri-drag-region>
              <LogoMark size={40} />
              <h1>{m.notice.bootFailed}</h1>
              <ErrorNote
                error={bootError}
                actions={
                  <Button size="sm" onClick={() => void useAppStore.getState().boot()}>
                    {m.common.retry}
                  </Button>
                }
              />
            </div>
          )}

          {ready && (
            <>
              <TopBar inset={macWindow} controls={ownsWindowFrame} tone={tone} />
              <div className={styles.body}>
                <Dock />
                <main className={styles.sheet}>
                  <Notices />
                  <div className={styles.stage} data-sky={tone ?? undefined}>
                  <AnimatePresence mode="wait" custom={direction} initial={false}>
                    <motion.div
                      key={viewKey}
                      className={styles.page}
                      custom={direction}
                      variants={pageVariants}
                      initial="initial"
                      animate="animate"
                      exit="exit"
                    >
                      {view.name === "projects" && <ProjectsPage />}
                      {view.name === "project" && <ProjectDetailPage projectId={view.projectId} tab={view.tab} />}
                      {view.name === "github" && <GithubPage />}
                      {view.name === "history" && <HistoryPage />}
                      {view.name === "settings" && <SettingsPage />}
                    </motion.div>
                    </AnimatePresence>
                  </div>
                </main>
              </div>
              <GlobalDialogs />
              <OperationSheet />
              <CommandPalette />
            </>
          )}

          {ownsWindowFrame && <WindowControls />}
          <ConfirmHost />
          <Toaster />
        </div>
      </TooltipProvider>
    </MotionConfig>
  );
}
