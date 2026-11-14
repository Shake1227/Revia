import { Slider as RadixSlider } from "radix-ui";
import styles from "./Slider.module.css";

interface SliderProps {
  value: number;
  onChange: (value: number) => void;
  min: number;
  max: number;
  step?: number;
  ariaLabel: string;
  display?: string;
  disabled?: boolean;
}

export function Slider({ value, onChange, min, max, step = 1, ariaLabel, display, disabled }: SliderProps) {
  return (
    <div className={styles.wrap}>
      <RadixSlider.Root
        className={styles.root}
        value={[value]}
        min={min}
        max={max}
        step={step}
        disabled={disabled}
        onValueChange={(values) => onChange(values[0] ?? value)}
      >
        <RadixSlider.Track className={styles.track}>
          <RadixSlider.Range className={styles.range} />
        </RadixSlider.Track>
        <RadixSlider.Thumb className={styles.thumb} aria-label={ariaLabel} />
      </RadixSlider.Root>
      {display !== undefined && <span className={`${styles.value} tabular`}>{display}</span>}
    </div>
  );
}
