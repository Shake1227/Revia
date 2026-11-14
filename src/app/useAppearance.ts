import { useEffect, useState } from "react";
import { isMac } from "../lib/platform";
import { useAppStore } from "../stores/app";

function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState(() => window.matchMedia(query).matches);
  useEffect(() => {
    const media = window.matchMedia(query);
    const update = () => setMatches(media.matches);
    update();
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, [query]);
  return matches;
}

export function useAppearance(skyTone: "light" | "dark" | null) {
  const theme = useAppStore((state) => state.ui.theme);
  const motion = useAppStore((state) => state.ui.motion);
  const typeface = useAppStore((state) => state.ui.typeface);
  const systemDark = useMediaQuery("(prefers-color-scheme: dark)");
  const systemReduced = useMediaQuery("(prefers-reduced-motion: reduce)");

  const system = systemDark ? "dark" : "light";
  const resolvedTheme = theme === "system" ? system : theme === "sky" ? (skyTone ?? system) : theme;
  const reducedMotion = motion === "system" ? systemReduced : motion === "reduced";

  useEffect(() => {
    const root = document.documentElement;
    root.dataset.theme = resolvedTheme;
    root.dataset.motion = reducedMotion ? "reduced" : "full";
    root.dataset.typeface = typeface;
    if (!root.dataset.platform) root.dataset.platform = isMac ? "macos" : "other";
  }, [resolvedTheme, reducedMotion, typeface]);

  return { resolvedTheme, reducedMotion };
}
