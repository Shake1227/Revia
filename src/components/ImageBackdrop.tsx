import type { CSSProperties } from "react";
import type { ImageFrame } from "../stores/app";
import styles from "./ImageBackdrop.module.css";

interface ImageBackdropProps {
  url: string;
  frame: ImageFrame | null;
  blur: number;
  size?: { width: number; height: number };
}

export function ImageBackdrop({ url, frame, blur, size }: ImageBackdropProps) {
  const stage = {
    ...(size ? { "--vw": `${size.width}px`, "--vh": `${size.height}px` } : {}),
    ...(frame ? { "--a": frame.aspect, "--fx": frame.x, "--fy": frame.y, "--fw": frame.w, "--fh": frame.h } : {}),
  } as CSSProperties;
  const soften: CSSProperties | undefined =
    blur > 0 ? { filter: `blur(${blur}px)`, ...(frame?.fit === "show" ? {} : { scale: "1.06" }) } : undefined;

  return (
    <div className={styles.stage} style={stage} data-fit={frame?.fit ?? "whole"}>
      <div className={styles.layers} style={soften}>
        {frame === null ? (
          <div className={styles.whole} style={{ backgroundImage: `url("${url}")` }} />
        ) : (
          <>
            {frame.fit === "show" && frame.edge && <div className={styles.edge} style={{ backgroundImage: `url("${frame.edge}")` }} />}
            <div className={styles.window}>
              <img className={styles.picture} src={url} alt="" draggable={false} />
            </div>
          </>
        )}
      </div>
    </div>
  );
}
