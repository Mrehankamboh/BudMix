import { getCurrentWindow } from "@tauri-apps/api/window";

/** The tray dock or the full settings window; both load this same page. */
export type View = "dock" | "settings";

export function currentView(): View {
  try {
    return getCurrentWindow().label === "dock" ? "dock" : "settings";
  } catch {
    // Outside Tauri (tests, a plain browser).
    return "settings";
  }
}
