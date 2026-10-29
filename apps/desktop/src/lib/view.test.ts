import { afterEach, describe, expect, it, vi } from "vitest";

const label = vi.hoisted(() => ({ value: "dock" as string | null }));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => {
    if (label.value === null) throw new Error("not running in Tauri");
    return { label: label.value };
  },
}));

import { currentView } from "./view";

afterEach(() => {
  label.value = "dock";
});

describe("currentView", () => {
  it("renders the dock in the dock window", () => {
    expect(currentView()).toBe("dock");
  });

  it("renders settings in the main window and outside Tauri", () => {
    label.value = "main";
    expect(currentView()).toBe("settings");
    label.value = null;
    expect(currentView()).toBe("settings");
  });
});
