export const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

export function previewPlatform(): "macos" | "windows" | "linux" | null {
  if (typeof window === "undefined") return null;
  const value = new URLSearchParams(window.location.search).get("platform");
  return value === "macos" || value === "windows" || value === "linux" ? value : null;
}

export function keyLabel(key: string): string {
  switch (key) {
    case "mod":
      return isMac ? "⌘" : "Ctrl";
    case "shift":
      return isMac ? "⇧" : "Shift";
    case "alt":
      return isMac ? "⌥" : "Alt";
    case "enter":
      return isMac ? "↩" : "Enter";
    case "esc":
      return "Esc";
    case "space":
      return "Space";
    case "up":
      return "↑";
    case "down":
      return "↓";
    case "backspace":
      return isMac ? "⌫" : "Backspace";
    default:
      return key.length === 1 ? key.toUpperCase() : key;
  }
}

export const isModKey = (event: KeyboardEvent | React.KeyboardEvent | MouseEvent | React.MouseEvent) =>
  isMac ? event.metaKey : event.ctrlKey;

export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}
