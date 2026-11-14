import { useId } from "react";
import styles from "./Logo.module.css";

interface LogoProps {
  size?: number;
  weight?: "auto" | "fine" | "bold";
  drawn?: boolean;
  className?: string;
}

export function LogoMark({ size = 24, weight = "auto", drawn, className }: LogoProps) {
  const id = useId();
  const bold = weight === "bold" || (weight === "auto" && size < 30);
  const stroke = bold ? 5 : 3.8;
  const node = bold ? 4.3 : 4.5;
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" fill="none" className={className} aria-hidden>
      <defs>
        <linearGradient id={id} x1="13" y1="7" x2="51" y2="57" gradientUnits="userSpaceOnUse">
          <stop offset="0" stopColor="var(--brand-1)" />
          <stop offset="0.5" stopColor="var(--brand-2)" />
          <stop offset="1" stopColor="var(--brand-3)" />
        </linearGradient>
      </defs>
      <g stroke={`url(#${id})`} strokeWidth={stroke} strokeLinecap="round" strokeLinejoin="round" className={drawn ? styles.drawn : undefined}>
        <circle cx="20" cy="13" r={node} pathLength={1} />
        <path d="M20 17.5v29" pathLength={1} />
        <path d="M24.5 13H32a11 11 0 0 1 0 22H20" pathLength={1} />
        <path d="M26 35c7.5 0 11.5 5.5 15.4 12.300" pathLength={1} />
        <circle cx="20" cy="51" r={node} pathLength={1} />
        <circle cx="44" cy="51" r={node} pathLength={1} />
      </g>
    </svg>
  );
}

export function Wordmark({ className }: { className?: string }) {
  return <span className={`${styles.wordmark} ${className ?? ""}`}>Revia</span>;
}
