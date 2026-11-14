import { create } from "zustand";
import { en } from "./en";
import { ja, type Messages } from "./ja";

export type Language = "ja" | "en";

const dictionaries: Record<Language, Messages> = { ja, en };

export function detectLanguage(): Language {
  const preferred = typeof navigator === "undefined" ? "" : (navigator.languages?.[0] ?? navigator.language ?? "");
  return preferred.toLowerCase().startsWith("ja") ? "ja" : "en";
}

const useLanguageStore = create<{ language: Language }>(() => ({ language: detectLanguage() }));

export function setLanguage(language: Language) {
  useLanguageStore.setState({ language });
  document.documentElement.lang = language;
}

export function useT(): Messages {
  return dictionaries[useLanguageStore((state) => state.language)];
}

export function t(): Messages {
  return dictionaries[useLanguageStore.getState().language];
}

export type { Messages };
