import clsx from "clsx";
import { Select as RadixSelect } from "radix-ui";
import type { ReactNode } from "react";
import { Icons } from "./icons";
import menuStyles from "./Menu.module.css";
import styles from "./Select.module.css";

export interface SelectOption<T extends string> {
  value: T;
  label: ReactNode;
  hint?: ReactNode;
  disabled?: boolean;
  group?: string;
}

interface SelectProps<T extends string> {
  value: T | undefined;
  onChange: (value: T) => void;
  options: SelectOption<T>[];
  placeholder?: string;
  size?: "sm" | "md";
  disabled?: boolean;
  ariaLabel?: string;
  leading?: ReactNode;
  className?: string;
  mono?: boolean;
  id?: string;
}

export function Select<T extends string>({
  value,
  onChange,
  options,
  placeholder,
  size = "md",
  disabled,
  ariaLabel,
  leading,
  className,
  mono,
  id,
}: SelectProps<T>) {
  return (
    <RadixSelect.Root value={value} onValueChange={(next) => onChange(next as T)} disabled={disabled}>
      <RadixSelect.Trigger id={id} className={clsx(styles.trigger, styles[size], className)} aria-label={ariaLabel}>
        {leading && <span className={styles.leading}>{leading}</span>}
        <span className={clsx(styles.value, mono && "mono")}>
          <RadixSelect.Value placeholder={placeholder} />
        </span>
        <RadixSelect.Icon className={styles.chevron}>
          <Icons.chevronsUpDown size={13} />
        </RadixSelect.Icon>
      </RadixSelect.Trigger>
      <RadixSelect.Portal>
        <RadixSelect.Content className={clsx(menuStyles.content, styles.content)} position="popper" sideOffset={6} collisionPadding={10}>
          <RadixSelect.Viewport>
            {options.map((option, index) => (
              <div key={option.value}>
                {option.group && option.group !== options[index - 1]?.group && <div className={menuStyles.label}>{option.group}</div>}
                <RadixSelect.Item value={option.value} disabled={option.disabled} className={clsx(menuStyles.item, styles.item)}>
                  <span className={menuStyles.text}>
                    <RadixSelect.ItemText>
                      <span className={mono ? "mono" : undefined}>{option.label}</span>
                    </RadixSelect.ItemText>
                    {option.hint && <span className={menuStyles.hint}>{option.hint}</span>}
                  </span>
                  <RadixSelect.ItemIndicator className={styles.indicator}>
                    <Icons.check size={14} />
                  </RadixSelect.ItemIndicator>
                </RadixSelect.Item>
              </div>
            ))}
          </RadixSelect.Viewport>
        </RadixSelect.Content>
      </RadixSelect.Portal>
    </RadixSelect.Root>
  );
}
