import { useSyncExternalStore } from "react";

import type { Theme } from "./ipc";

let preference: Theme = "system";
const listeners = new Set<() => void>();
const media = typeof window !== "undefined" ? window.matchMedia("(prefers-color-scheme: dark)") : null;

function resolved(): "light" | "dark" {
  if (preference === "system") return media?.matches ? "dark" : "light";
  return preference;
}

function apply() {
  document.documentElement.classList.toggle("dark", resolved() === "dark");
  document.documentElement.style.colorScheme = resolved();
  listeners.forEach((l) => l());
}

media?.addEventListener("change", () => preference === "system" && apply());

export function setThemePreference(theme: Theme) {
  preference = theme;
  apply();
}

export function useResolvedTheme(): "light" | "dark" {
  return useSyncExternalStore(
    (cb) => {
      listeners.add(cb);
      return () => listeners.delete(cb);
    },
    resolved,
  );
}

export const isMac = typeof navigator !== "undefined" && /Mac/i.test(navigator.platform || navigator.userAgent);
