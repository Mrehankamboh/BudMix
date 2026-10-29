import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { DeviceView } from "../lib/types";
import { DeviceCard } from "./DeviceCard";

const device: DeviceView = {
  id: "{buds}",
  name: "Test Buds",
  profiles: ["stereo", "handsFree"],
  state: "disconnected",
  reconnectAttempt: null,
  lastError: null,
  lastConnected: false,
};

function renderCard(overrides: Partial<DeviceView> = {}) {
  const onToggle = vi.fn();
  const onMakeHotkeyTarget = vi.fn();
  render(
    <ul>
      <DeviceCard
        device={{ ...device, ...overrides }}
        isHotkeyTarget={false}
        onToggle={onToggle}
        onMakeHotkeyTarget={onMakeHotkeyTarget}
      />
    </ul>,
  );
  return { onToggle, onMakeHotkeyTarget };
}

describe("DeviceCard", () => {
  it("offers Connect for a disconnected device", async () => {
    const { onToggle } = renderCard();
    expect(screen.getByText("Not connected")).toBeInTheDocument();
    expect(screen.getByText("Hands-free")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Connect Test Buds" }));
    expect(onToggle).toHaveBeenCalledWith(expect.objectContaining({ id: "{buds}" }));
  });

  it("offers Disconnect while connected or connecting", () => {
    renderCard({ state: "connecting" });
    expect(screen.getByRole("button", { name: "Disconnect Test Buds" })).toBeInTheDocument();
    expect(screen.getByRole("listitem", { busy: true })).toBeInTheDocument();
  });

  it("shows the last error", () => {
    renderCard({ lastError: "Connection timed out." });
    expect(screen.getByRole("alert")).toHaveTextContent("Connection timed out.");
  });

  it("selects the hotkey target", async () => {
    const { onMakeHotkeyTarget } = renderCard();
    await userEvent.click(screen.getByRole("radio", { name: "Hotkey target" }));
    expect(onMakeHotkeyTarget).toHaveBeenCalledWith("{buds}");
  });
});
