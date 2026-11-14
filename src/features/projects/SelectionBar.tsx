import { AnimatePresence, motion } from "motion/react";
import { Button, IconButton } from "../../components/Button";
import { Icons } from "../../components/icons";
import { Kbd } from "../../components/Kbd";
import { Menu, MenuContent, MenuItem, MenuSeparator, MenuTrigger } from "../../components/Menu";
import { Tooltip } from "../../components/Tooltip";
import { spring } from "../../design/motion";
import { useT } from "../../i18n";
import { notifyError } from "../../lib/notify";
import { useOperationStore } from "../../stores/operations";
import { useProjectStore } from "../../stores/projects";
import { useUiStore } from "../../stores/ui";
import { batchRequest } from "./model";
import styles from "./SelectionBar.module.css";

export function SelectionBar({ visibleIds }: { visibleIds: string[] }) {
  const m = useT();
  const selected = useProjectStore((state) => state.selected);
  const ids = Object.keys(selected);
  const count = ids.length;
  const prepare = useOperationStore((state) => state.prepare);
  const openDialog = useUiStore((state) => state.openDialog);
  const clear = useProjectStore((state) => state.clearSelection);

  return (
    <AnimatePresence>
      {count > 0 && (
        <motion.div
          className={styles.wrap}
          initial={{ opacity: 0, y: 28, scale: 0.96 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: 18, scale: 0.98, transition: { duration: 0.14 } }}
          transition={spring.smooth}
        >
          <div className={styles.bar} data-surface role="toolbar" aria-label={m.projects.selection.selected(count)}>
            <span className={styles.count}>
              <span className={`${styles.number} tabular`}>{count}</span>
              <span>{m.projects.selection.selected(count).replace(String(count), "").trim()}</span>
            </span>
            <span className={styles.divider} />
            <Button variant="primary" size="sm" icon={<Icons.refresh size={13} />} onClick={() => void prepare(batchRequest("sync", ids))}>
              {m.action.sync}
            </Button>
            <Tooltip label={m.action.fetch} hint={m.actionHint.fetch} side="top">
              <Button variant="ghost" size="sm" onClick={() => void prepare(batchRequest("fetch", ids))}>
                {m.action.fetch}
              </Button>
            </Tooltip>
            <Tooltip label={m.action.pull} hint={m.actionHint.pull} side="top">
              <Button variant="ghost" size="sm" onClick={() => void prepare(batchRequest("pull", ids))}>
                {m.action.pull}
              </Button>
            </Tooltip>
            <Tooltip label={m.action.push} hint={m.actionHint.push} side="top">
              <Button variant="ghost" size="sm" onClick={() => void prepare(batchRequest("push", ids))}>
                {m.action.push}
              </Button>
            </Tooltip>
            <Button variant="ghost" size="sm" onClick={() => openDialog({ type: "batchCommit", projectIds: ids })}>
              {m.action.commit}…
            </Button>
            <Menu>
              <MenuTrigger asChild>
                <IconButton label={m.common.more} size="sm" noTooltip>
                  <Icons.ellipsis size={15} />
                </IconButton>
              </MenuTrigger>
              <MenuContent side="top" align="end">
                <MenuItem icon={<Icons.merge size={14} />} onSelect={() => openDialog({ type: "batchMerge", projectIds: ids })}>
                  {m.action.merge}…
                </MenuItem>
                <MenuItem icon={<Icons.github size={14} />} onSelect={() => openDialog({ type: "publish", projectIds: ids })}>
                  {m.action.publish}…
                </MenuItem>
                <MenuSeparator />
                <MenuItem
                  icon={<Icons.retry size={14} />}
                  onSelect={() => void useProjectStore.getState().refresh(ids).catch(notifyError)}
                >
                  {m.action.refresh}
                </MenuItem>
                <MenuItem
                  icon={<Icons.check size={14} />}
                  shortcut={["mod", "A"]}
                  onSelect={() => useProjectStore.getState().selectMany(visibleIds)}
                >
                  {m.projects.selection.selectAll}
                </MenuItem>
              </MenuContent>
            </Menu>
            <span className={styles.divider} />
            <Tooltip label={m.projects.selection.clear} shortcut={["esc"]} side="top">
              <button type="button" className={styles.clear} onClick={clear} aria-label={m.projects.selection.clear}>
                <Icons.x size={14} />
                <Kbd keys={["esc"]} />
              </button>
            </Tooltip>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
