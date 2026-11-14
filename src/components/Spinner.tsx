import styles from "./Spinner.module.css";

interface SpinnerProps {
  size?: number;
  progress?: number | null;
  className?: string;
}

export function Spinner({ size = 14, progress = null, className }: SpinnerProps) {
  const radius = 6;
  const circumference = 2 * Math.PI * radius;
  const determinate = progress !== null && progress !== undefined;
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      className={`${styles.spinner} ${determinate ? styles.determinate : ""} ${className ?? ""}`}
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={1}
      aria-valuenow={determinate ? progress : undefined}
    >
      <circle cx="8" cy="8" r={radius} className={styles.track} strokeWidth="1.8" />
      <circle
        cx="8"
        cy="8"
        r={radius}
        className={styles.arc}
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeDasharray={circumference}
        strokeDashoffset={determinate ? circumference * (1 - Math.min(1, Math.max(0.04, progress))) : circumference * 0.68}
      />
    </svg>
  );
}
