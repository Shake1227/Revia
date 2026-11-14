import { Switch as RadixSwitch } from "radix-ui";
import styles from "./Switch.module.css";

interface SwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  ariaLabel?: string;
  id?: string;
}

export function Switch({ checked, onChange, disabled, ariaLabel, id }: SwitchProps) {
  return (
    <RadixSwitch.Root id={id} className={styles.root} checked={checked} onCheckedChange={onChange} disabled={disabled} aria-label={ariaLabel}>
      <RadixSwitch.Thumb className={styles.thumb} />
    </RadixSwitch.Root>
  );
}
