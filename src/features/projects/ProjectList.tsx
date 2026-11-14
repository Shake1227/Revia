import { useVirtualizer } from "@tanstack/react-virtual";
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useMemo, useRef, type KeyboardEvent, type RefObject } from "react";
import { listItemVariants, spring } from "../../design/motion";
import { useT } from "../../i18n";
import type { Project } from "../../ipc/types";
import { isModKey } from "../../lib/platform";
import { useProjectStore } from "../../stores/projects";
import { useUiStore } from "../../stores/ui";
import styles from "./ProjectList.module.css";
import { ProjectRow } from "./ProjectRow";

interface ProjectListProps {
  projects: Project[];
  compact: boolean;
  scrollRef: RefObject<HTMLDivElement | null>;
}

const VIRTUALIZE_FROM = 70;
const GAP = 2;

export function ProjectList({ projects, compact, scrollRef }: ProjectListProps) {
  const m = useT();
  const orderedIds = useMemo(() => projects.map((project) => project.id), [projects]);
  const selectionMode = useProjectStore((state) => Object.keys(state.selected).length > 0);
  const cursorId = useProjectStore((state) => state.cursorId);
  const listRef = useRef<HTMLDivElement>(null);
  const rowHeight = compact ? 40 : 54;
  const virtual = projects.length > VIRTUALIZE_FROM;

  const virtualizer = useVirtualizer({
    count: virtual ? projects.length : 0,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => rowHeight + GAP,
    overscan: 10,
    scrollMargin: listRef.current?.offsetTop ?? 0,
  });

  const moveCursor = useCallback(
    (delta: number, extend: boolean) => {
      const store = useProjectStore.getState();
      const current = store.cursorId ? orderedIds.indexOf(store.cursorId) : -1;
      const next = Math.min(orderedIds.length - 1, Math.max(0, current < 0 ? (delta > 0 ? 0 : orderedIds.length - 1) : current + delta));
      const id = orderedIds[next];
      if (!id) return;
      store.setCursor(id);
      if (extend) store.selectRange(id, orderedIds);
      if (virtual) virtualizer.scrollToIndex(next, { align: "auto" });
      else document.getElementById(`project-row-${id}`)?.scrollIntoView({ block: "nearest" });
    },
    [orderedIds, virtual, virtualizer],
  );

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const store = useProjectStore.getState();
    const key = event.key;
    if (key === "ArrowDown" || (key === "j" && !isModKey(event))) {
      event.preventDefault();
      moveCursor(1, event.shiftKey);
    } else if (key === "ArrowUp" || (key === "k" && !isModKey(event))) {
      event.preventDefault();
      moveCursor(-1, event.shiftKey);
    } else if (key === "Home") {
      event.preventDefault();
      moveCursor(-orderedIds.length, false);
    } else if (key === "End") {
      event.preventDefault();
      moveCursor(orderedIds.length, false);
    } else if (key === " " && store.cursorId) {
      event.preventDefault();
      store.toggle(store.cursorId);
    } else if (key === "Enter" && store.cursorId) {
      event.preventDefault();
      useUiStore.getState().openProject(store.cursorId);
    } else if (key === "a" && isModKey(event)) {
      event.preventDefault();
      store.selectMany(orderedIds);
    }
  };

  useEffect(() => {
    if (cursorId && !orderedIds.includes(cursorId)) useProjectStore.getState().setCursor(null);
  }, [cursorId, orderedIds]);

  const common = {
    ref: listRef,
    id: "project-list",
    role: "listbox" as const,
    "aria-label": m.projects.title,
    "aria-multiselectable": true,
    "aria-activedescendant": cursorId ? `project-row-${cursorId}` : undefined,
    tabIndex: 0,
    className: styles.list,
    onKeyDown,
    onBlur: () => undefined,
  };

  if (virtual) {
    return (
      <div {...common} style={{ height: virtualizer.getTotalSize() }}>
        {virtualizer.getVirtualItems().map((item) => {
          const project = projects[item.index]!;
          return (
            <div
              key={project.id}
              className={styles.virtualRow}
              style={{ transform: `translateY(${item.start - virtualizer.options.scrollMargin}px)`, height: rowHeight }}
            >
              <ProjectRow projectId={project.id} orderedIds={orderedIds} selectionMode={selectionMode} compact={compact} />
            </div>
          );
        })}
      </div>
    );
  }

  return (
    <div {...common}>
      <AnimatePresence initial mode="popLayout">
        {projects.map((project, index) => (
          <motion.div
            key={project.id}
            layout="position"
            custom={index}
            variants={listItemVariants}
            initial="hidden"
            animate="visible"
            exit="exit"
            transition={spring.smooth}
            className={styles.item}
          >
            <ProjectRow projectId={project.id} orderedIds={orderedIds} selectionMode={selectionMode} compact={compact} />
          </motion.div>
        ))}
      </AnimatePresence>
    </div>
  );
}
