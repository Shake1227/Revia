import clsx from "clsx";
import { DropdownMenu } from "radix-ui";
import type { ReactNode } from "react";
import { Icons } from "./icons";
import { Kbd } from "./Kbd";
import styles from "./Menu.module.css";

export const Menu = DropdownMenu.Root;
export const MenuTrigger = DropdownMenu.Trigger;
export const MenuGroup = DropdownMenu.Group;
export const MenuRadioGroup = DropdownMenu.RadioGroup;

interface MenuContentProps {
  children: ReactNode;
  align?: "start" | "center" | "end";
  side?: "top" | "right" | "bottom" | "left";
  minWidth?: number;
  className?: string;
}

export function MenuContent({ children, align = "end", side = "bottom", minWidth = 200, className }: MenuContentProps) {
  return (
    <DropdownMenu.Portal>
      <DropdownMenu.Content
        className={clsx(styles.content, className)}
        align={align}
        side={side}
        sideOffset={6}
        collisionPadding={10}
        style={{ minWidth }}
        onCloseAutoFocus={(event) => event.preventDefault()}
      >
        {children}
      </DropdownMenu.Content>
    </DropdownMenu.Portal>
  );
}

interface MenuItemProps {
  icon?: ReactNode;
  children: ReactNode;
  hint?: ReactNode;
  shortcut?: string[];
  danger?: boolean;
  disabled?: boolean;
  onSelect?: () => void;
}

export function MenuItem({ icon, children, hint, shortcut, danger, disabled, onSelect }: MenuItemProps) {
  return (
    <DropdownMenu.Item className={styles.item} data-danger={danger || undefined} disabled={disabled} onSelect={onSelect}>
      {icon !== undefined && <span className={styles.icon}>{icon}</span>}
      <span className={styles.text}>
        <span>{children}</span>
        {hint && <span className={styles.hint}>{hint}</span>}
      </span>
      {shortcut && <Kbd keys={shortcut} className={styles.shortcut} />}
    </DropdownMenu.Item>
  );
}

interface MenuCheckItemProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  children: ReactNode;
}

export function MenuCheckItem({ checked, onCheckedChange, children }: MenuCheckItemProps) {
  return (
    <DropdownMenu.CheckboxItem
      className={styles.item}
      checked={checked}
      onCheckedChange={onCheckedChange}
      onSelect={(event) => event.preventDefault()}
    >
      <span className={styles.icon}>
        <DropdownMenu.ItemIndicator>
          <Icons.check size={14} />
        </DropdownMenu.ItemIndicator>
      </span>
      <span className={styles.text}>{children}</span>
    </DropdownMenu.CheckboxItem>
  );
}

export function MenuRadioItem({ value, children, hint }: { value: string; children: ReactNode; hint?: ReactNode }) {
  return (
    <DropdownMenu.RadioItem className={styles.item} value={value}>
      <span className={styles.icon}>
        <DropdownMenu.ItemIndicator>
          <Icons.check size={14} />
        </DropdownMenu.ItemIndicator>
      </span>
      <span className={styles.text}>
        <span>{children}</span>
        {hint && <span className={styles.hint}>{hint}</span>}
      </span>
    </DropdownMenu.RadioItem>
  );
}

export function MenuLabel({ children }: { children: ReactNode }) {
  return <DropdownMenu.Label className={styles.label}>{children}</DropdownMenu.Label>;
}

export function MenuSeparator() {
  return <DropdownMenu.Separator className={styles.separator} />;
}
