import { motion } from "motion/react";
import { Icons, type IconComponent } from "../../components/icons";
import { stateVisual } from "../../components/StatusGlyph";
import { Tooltip } from "../../components/Tooltip";
import { spring } from "../../design/motion";
import { useT } from "../../i18n";
import { useUiStore, type View } from "../../stores/ui";
import { useUpdateStore } from "../../stores/update";
import { countSummary } from "../projects/model";
import { useAllProjects } from "../projects/useVisibleProjects";
import styles from "./Dock.module.css";

interface NavEntry {
  name: Exclude<View["name"], "project" | "excluded">;
  label: string;
  icon: IconComponent;
  shortcut: string[];
}

export function Dock() {
  const m = useT();
  const view = useUiStore((state) => state.view);
  const navigate = useUiStore((state) => state.navigate);
  const openProject = useUiStore((state) => state.openProject);
  const openDialog = useUiStore((state) => state.openDialog);
  const projects = useAllProjects();
  const updateAvailable = useUpdateStore((state) => state.info?.available === true);

  const active = view.name === "project" ? "projects" : view.name === "excluded" ? "settings" : view.name;
  const attention = countSummary(projects).attention;
  const favorites = projects.filter((project) => project.settings.favorite).slice(0, 6);

  const entries: NavEntry[] = [
    { name: "projects", label: m.nav.projects, icon: Icons.layers, shortcut: ["mod", "1"] },
    { name: "github", label: m.nav.github, icon: Icons.github, shortcut: ["mod", "2"] },
    { name: "history", label: m.nav.history, icon: Icons.history, shortcut: ["mod", "3"] },
    { name: "settings", label: m.nav.settings, icon: Icons.settings, shortcut: ["mod", ","] },
  ];

  return (
    <nav className={styles.dock} aria-label={m.app.name}>
      {entries.map((entry) => {
        const selected = active === entry.name;
        const Icon = entry.icon;
        return (
          <Tooltip key={entry.name} label={entry.label} shortcut={entry.shortcut} side="right">
            <button
              type="button"
              className={styles.item}
              data-selected={selected || undefined}
              aria-current={selected ? "page" : undefined}
              aria-label={entry.label}
              onClick={() => navigate({ name: entry.name })}
            >
              {selected && <motion.span layoutId="dock-active" className={styles.active} transition={spring.snappy} />}
              <span className={styles.icon}>
                <Icon size={17} />
              </span>
              {entry.name === "projects" && attention > 0 && <span className={styles.badge} />}
              {entry.name === "settings" && updateAvailable && <span className={styles.badge} data-tone="update" />}
            </button>
          </Tooltip>
        );
      })}

      {favorites.length > 0 && <span className={styles.separator} />}
      {favorites.map((project) => {
        const tone = project.status ? stateVisual[project.status.state].tone : "neutral";
        const selected = view.name === "project" && view.projectId === project.id;
        return (
          <Tooltip key={project.id} label={project.name} hint={project.status ? m.state[project.status.state] : undefined} side="right">
            <button
              type="button"
              className={styles.orb}
              data-tone={tone}
              data-selected={selected || undefined}
              aria-label={project.name}
              onClick={() => openProject(project.id)}
            >
              {[...project.name][0]?.toUpperCase()}
            </button>
          </Tooltip>
        );
      })}

      <span className={styles.separator} />
      <Tooltip label={m.add.title} shortcut={["mod", "N"]} side="right">
        <button type="button" className={styles.item} aria-label={m.add.title} onClick={() => openDialog({ type: "add" })}>
          <span className={styles.icon}>
            <Icons.plus size={17} />
          </span>
        </button>
      </Tooltip>
    </nav>
  );
}
