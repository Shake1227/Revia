import { useEffect } from "react";
import { notifyError } from "../lib/notify";
import { isModKey } from "../lib/platform";
import { useConfirmStore } from "../stores/confirm";
import { useOperationStore } from "../stores/operations";
import { useProjectStore } from "../stores/projects";
import { useUiStore } from "../stores/ui";
import { batchRequest } from "../features/projects/model";
import { SEARCH_INPUT_ID } from "../features/projects/ProjectsPage";

const isTyping = (target: EventTarget | null) =>
  target instanceof HTMLElement && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable);

export const overlayOpen = () =>
  useUiStore.getState().dialog !== null ||
  useUiStore.getState().paletteOpen ||
  useOperationStore.getState().sheet !== null ||
  useConfirmStore.getState().current !== null ||
  document.querySelector('[role="menu"], [role="listbox"][data-state="open"]') !== null;

export function syncAllProjects() {
  const { byId, ids } = useProjectStore.getState();
  const targets = ids.filter((id) => {
    const project = byId[id];
    return (
      project?.settings.includeInBatch &&
      project.status !== null &&
      !["missing", "not_initialized", "no_remote"].includes(project.status.state)
    );
  });
  if (targets.length > 0) void useOperationStore.getState().prepare(batchRequest("sync", targets));
}

export function fetchAllProjects() {
  const { byId, ids } = useProjectStore.getState();
  const targets = ids.filter((id) => {
    const project = byId[id];
    return project?.settings.includeInBatch && project.status !== null && project.status.remotes.length > 0;
  });
  if (targets.length > 0) void useOperationStore.getState().prepare(batchRequest("fetch", targets));
}

export function togglePalette() {
  const ui = useUiStore.getState();
  if (ui.dialog === null && useOperationStore.getState().sheet === null && useConfirmStore.getState().current === null) {
    ui.setPaletteOpen(!ui.paletteOpen);
  }
}

export function focusProjectSearch() {
  const ui = useUiStore.getState();
  if (ui.view.name !== "projects") ui.navigate({ name: "projects" });
  const tryFocus = (attempt: number) => {
    const input = document.getElementById(SEARCH_INPUT_ID) as HTMLInputElement | null;
    if (input) {
      input.focus();
      input.select();
    } else if (attempt < 20) {
      setTimeout(() => tryFocus(attempt + 1), 30);
    }
  };
  tryFocus(0);
}

export function useShortcuts() {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const ui = useUiStore.getState();
      const mod = isModKey(event);
      const key = event.key.toLowerCase();

      if (mod && key === "k") {
        event.preventDefault();
        togglePalette();
        return;
      }
      if (overlayOpen()) return;

      if (mod) {
        if (key === "f") {
          event.preventDefault();
          focusProjectSearch();
        } else if (key === "r") {
          event.preventDefault();
          void useProjectStore.getState().refresh().catch(notifyError);
        } else if (key === "1") {
          event.preventDefault();
          ui.navigate({ name: "projects" });
        } else if (key === "2") {
          event.preventDefault();
          ui.navigate({ name: "github" });
        } else if (key === "3") {
          event.preventDefault();
          ui.navigate({ name: "history" });
        } else if (key === ",") {
          event.preventDefault();
          ui.navigate({ name: "settings" });
        } else if (key === "n") {
          event.preventDefault();
          ui.openDialog({ type: "add" });
        } else if (key === "s" && event.shiftKey) {
          event.preventDefault();
          syncAllProjects();
        } else if (key === "[") {
          event.preventDefault();
          ui.back();
        }
        return;
      }

      if (event.key === "Escape" && !isTyping(event.target)) {
        const projects = useProjectStore.getState();
        if (Object.keys(projects.selected).length > 0) projects.clearSelection();
        else if (ui.view.name === "project") ui.back();
        return;
      }

      if (ui.view.name === "projects" && !isTyping(event.target) && (event.key === "ArrowDown" || event.key === "ArrowUp")) {
        const list = document.getElementById("project-list");
        if (list && document.activeElement !== list && !list.contains(document.activeElement)) {
          event.preventDefault();
          list.focus();
          list.dispatchEvent(new KeyboardEvent("keydown", { key: event.key, bubbles: true }));
        }
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}
