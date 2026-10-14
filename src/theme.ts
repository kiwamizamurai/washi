import type { Prefs } from "./prefs";

export function applyPrefs(root: HTMLElement, prefs: Prefs) {
  if (prefs.theme === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", prefs.theme);
  root.setAttribute("data-width", prefs.width);
}

export function isDark(root: HTMLElement = document.documentElement) {
  const theme = root.getAttribute("data-theme");
  if (theme) return theme === "dark";
  return matchMedia("(prefers-color-scheme: dark)").matches;
}
