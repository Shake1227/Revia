import clsx from "clsx";
import { AnimatePresence, motion } from "motion/react";
import { Dialog as RadixDialog } from "radix-ui";
import type { ReactNode } from "react";
import { dialogVariants, overlayVariants } from "../design/motion";
import { useT } from "../i18n";
import { IconButton } from "./Button";
import styles from "./Dialog.module.css";
import { Icons } from "./icons";

interface DialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: ReactNode;
  description?: ReactNode;
  icon?: ReactNode;
  size?: "sm" | "md" | "lg" | "xl";
  footer?: ReactNode;
  footerStart?: ReactNode;
  children?: ReactNode;
  dismissible?: boolean;
  flush?: boolean;
  tone?: "default" | "danger";
}

export function Dialog({
  open,
  onOpenChange,
  title,
  description,
  icon,
  size = "md",
  footer,
  footerStart,
  children,
  dismissible = true,
  flush,
  tone = "default",
}: DialogProps) {
  const m = useT();
  return (
    <RadixDialog.Root open={open} onOpenChange={onOpenChange}>
      <AnimatePresence>
        {open && (
          <RadixDialog.Portal forceMount>
            <RadixDialog.Overlay asChild forceMount>
              <motion.div className={styles.overlay} variants={overlayVariants} initial="initial" animate="animate" exit="exit" />
            </RadixDialog.Overlay>
            <div className={styles.positioner}>
              <RadixDialog.Content
                asChild
                forceMount
                aria-describedby={undefined}
                onPointerDownOutside={(event) => !dismissible && event.preventDefault()}
                onEscapeKeyDown={(event) => !dismissible && event.preventDefault()}
              >
                <motion.div
                  className={clsx(styles.dialog, styles[size])}
                  data-tone={tone}
                  variants={dialogVariants}
                  initial="initial"
                  animate="animate"
                  exit="exit"
                >
                  <header className={styles.header}>
                    {icon && <span className={styles.icon}>{icon}</span>}
                    <div className={styles.heading}>
                      <RadixDialog.Title className={styles.title}>{title}</RadixDialog.Title>
                      {description && <RadixDialog.Description className={styles.description}>{description}</RadixDialog.Description>}
                    </div>
                    {dismissible && (
                      <RadixDialog.Close asChild>
                        <IconButton label={m.common.close} size="sm" className={styles.close} noTooltip>
                          <Icons.x size={15} />
                        </IconButton>
                      </RadixDialog.Close>
                    )}
                  </header>
                  {children && <div className={clsx(styles.body, flush && styles.flush)}>{children}</div>}
                  {(footer || footerStart) && (
                    <footer className={styles.footer}>
                      <div className={styles.footerStart}>{footerStart}</div>
                      <div className={styles.footerEnd}>{footer}</div>
                    </footer>
                  )}
                </motion.div>
              </RadixDialog.Content>
            </div>
          </RadixDialog.Portal>
        )}
      </AnimatePresence>
    </RadixDialog.Root>
  );
}
