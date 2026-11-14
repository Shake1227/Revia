import { useEffect, useState } from "react";
import { currentHour } from "../components/lake";

export function useSkyHour(auto: boolean, fixed: number): number {
  const [now, setNow] = useState(() => currentHour());

  useEffect(() => {
    if (!auto) return;
    let timer = 0;
    const update = () => setNow(currentHour());
    const schedule = () => {
      timer = window.setTimeout(() => {
        update();
        schedule();
      }, 60_000 - (Date.now() % 60_000) + 20);
    };
    update();
    schedule();
    window.addEventListener("focus", update);
    document.addEventListener("visibilitychange", update);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener("focus", update);
      document.removeEventListener("visibilitychange", update);
    };
  }, [auto]);

  return auto ? now : fixed;
}
