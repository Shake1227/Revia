import { keyLabel } from "../lib/platform";
import styles from "./Kbd.module.css";

export function Kbd({ keys, className }: { keys: string[]; className?: string }) {
  return (
    <span className={`${styles.kbd} ${className ?? ""}`}>
      {keys.map((key) => (
        <kbd key={key}>{keyLabel(key)}</kbd>
      ))}
    </span>
  );
}
