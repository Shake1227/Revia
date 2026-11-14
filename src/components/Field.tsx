import clsx from "clsx";
import { forwardRef, useId, type InputHTMLAttributes, type ReactNode, type TextareaHTMLAttributes } from "react";
import styles from "./Field.module.css";
import { Icons } from "./icons";

interface FieldProps {
  label?: ReactNode;
  hint?: ReactNode;
  error?: ReactNode;
  htmlFor?: string;
  children: ReactNode;
  className?: string;
}

export function Field({ label, hint, error, htmlFor, children, className }: FieldProps) {
  return (
    <div className={clsx(styles.field, className)}>
      {label && (
        <label className={styles.label} htmlFor={htmlFor}>
          {label}
        </label>
      )}
      {children}
      {error ? <p className={styles.error}>{error}</p> : hint ? <p className={styles.hint}>{hint}</p> : null}
    </div>
  );
}

interface TextFieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "size"> {
  leading?: ReactNode;
  trailing?: ReactNode;
  invalid?: boolean;
  mono?: boolean;
  fieldSize?: "sm" | "md" | "lg";
}

export const TextField = forwardRef<HTMLInputElement, TextFieldProps>(function TextField(
  { leading, trailing, invalid, mono, fieldSize = "md", className, ...props },
  ref,
) {
  return (
    <div className={clsx(styles.control, styles[fieldSize], className)} data-invalid={invalid || undefined} data-disabled={props.disabled || undefined}>
      {leading && <span className={styles.leading}>{leading}</span>}
      <input ref={ref} className={clsx(styles.input, mono && "mono")} spellCheck={false} autoComplete="off" autoCorrect="off" autoCapitalize="off" {...props} />
      {trailing && <span className={styles.trailing}>{trailing}</span>}
    </div>
  );
});

interface TextAreaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  invalid?: boolean;
}

export const TextArea = forwardRef<HTMLTextAreaElement, TextAreaProps>(function TextArea({ invalid, className, ...props }, ref) {
  return (
    <div className={clsx(styles.control, styles.area, className)} data-invalid={invalid || undefined}>
      <textarea ref={ref} className={styles.textarea} spellCheck={false} {...props} />
    </div>
  );
});

interface SearchFieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "onChange" | "value" | "size"> {
  value: string;
  onChange: (value: string) => void;
  trailing?: ReactNode;
  clearLabel: string;
}

export const SearchField = forwardRef<HTMLInputElement, SearchFieldProps>(function SearchField(
  { value, onChange, trailing, clearLabel, className, ...props },
  ref,
) {
  const id = useId();
  return (
    <TextField
      ref={ref}
      id={id}
      type="text"
      role="searchbox"
      value={value}
      onChange={(event) => onChange(event.target.value)}
      onKeyDown={(event) => {
        if (event.key === "Escape" && value) {
          event.stopPropagation();
          onChange("");
        }
        props.onKeyDown?.(event);
      }}
      leading={<Icons.search size={14} />}
      trailing={
        value ? (
          <button type="button" className={styles.clear} aria-label={clearLabel} onClick={() => onChange("")}>
            <Icons.x size={12} />
          </button>
        ) : (
          trailing
        )
      }
      className={clsx(styles.search, className)}
      {...props}
    />
  );
});
