import { animate } from "motion/react";
import { useEffect, useRef, useState } from "react";

export function CountUp({ value }: { value: number }) {
  const [shown, setShown] = useState(0);
  const previous = useRef(0);

  useEffect(() => {
    const controls = animate(previous.current, value, {
      duration: 0.7,
      ease: [0.16, 1, 0.3, 1],
      onUpdate: (latest) => setShown(Math.round(latest)),
    });
    previous.current = value;
    return () => controls.stop();
  }, [value]);

  return <span className="tabular">{shown}</span>;
}
