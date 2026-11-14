import { useState } from "react";
import styles from "./Avatar.module.css";

interface AvatarProps {
  name: string;
  src?: string | null;
  size?: number;
}

export function Avatar({ name, src, size = 24 }: AvatarProps) {
  const [failed, setFailed] = useState(false);
  return (
    <span className={styles.avatar} style={{ width: size, height: size, fontSize: Math.round(size * 0.44) }}>
      {src && !failed ? <img src={src} alt="" width={size} height={size} onError={() => setFailed(true)} draggable={false} /> : name.slice(0, 1).toUpperCase()}
    </span>
  );
}
