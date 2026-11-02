import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { api } from "../../lib/api";
import type { CastStatus } from "../../lib/types";
import { CastSoundPrompt } from "./CastSoundPrompt";

vi.mock("../../lib/api", () => ({
  api: {
    castSoundAllowed: vi.fn(),
    dockKeepOpen: vi.fn(),
    runFix: vi.fn(),
  },
}));

const mocked = vi.mocked(api);

const playing: CastStatus = {
  receiver: "ready",
  message: null,
  phone: "Infinix",
  phase: "playing",
  pin: null,
  allowed: [],
  mode: "auto",
  bluetoothSuspended: true,
  videoVisible: true,
  lastError: null,
};

beforeEach(() => {
  vi.clearAllMocks();
  mocked.castSoundAllowed.mockResolvedValue(false);
  mocked.dockKeepOpen.mockResolvedValue(undefined);
  mocked.runFix.mockResolvedValue(undefined);
});

describe("CastSoundPrompt", () => {
  it("stays out of the way while no phone casts", () => {
    const { container } = render(<CastSoundPrompt cast={{ ...playing, phase: "idle" }} />);
    expect(container).toBeEmptyDOMElement();
    expect(mocked.castSoundAllowed).not.toHaveBeenCalled();
  });

  it("waits for the phone's approval first", () => {
    render(<CastSoundPrompt cast={{ ...playing, phase: "needsApproval" }} />);
    expect(mocked.castSoundAllowed).not.toHaveBeenCalled();
  });

  it("says nothing when the firewall already lets the sound through", async () => {
    mocked.castSoundAllowed.mockResolvedValue(true);
    render(<CastSoundPrompt cast={playing} />);
    await waitFor(() => expect(mocked.castSoundAllowed).toHaveBeenCalled());
    expect(screen.queryByRole("button", { name: "Allow sound" })).not.toBeInTheDocument();
  });

  it("explains what the rule does and keeps the dock open while it asks", async () => {
    render(<CastSoundPrompt cast={playing} />);
    expect(await screen.findByRole("button", { name: "Allow sound" })).toBeInTheDocument();
    expect(screen.getByText(/stopping the sound from Infinix/)).toHaveTextContent("also on public Wi-Fi");
    await waitFor(() => expect(mocked.dockKeepOpen).toHaveBeenLastCalledWith(true));
  });

  it("adds the rule on one click and lets the dock close again", async () => {
    render(<CastSoundPrompt cast={playing} />);
    const allow = await screen.findByRole("button", { name: "Allow sound" });
    mocked.castSoundAllowed.mockResolvedValue(true);
    await userEvent.click(allow);
    expect(mocked.runFix).toHaveBeenCalledWith({ kind: "addFirewallRule" });
    expect(await screen.findByRole("status")).toHaveTextContent("Sound allowed");
    await waitFor(() => expect(mocked.dockKeepOpen).toHaveBeenLastCalledWith(false));
  });

  it("asks again when the Windows prompt is cancelled", async () => {
    mocked.runFix.mockRejectedValue("The admin prompt was cancelled, so nothing changed.");
    render(<CastSoundPrompt cast={playing} />);
    await userEvent.click(await screen.findByRole("button", { name: "Allow sound" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("cancelled");
    expect(screen.getByRole("button", { name: "Allow sound" })).toBeEnabled();
  });

  it("never claims success while Windows still blocks the sound", async () => {
    render(<CastSoundPrompt cast={playing} />);
    await userEvent.click(await screen.findByRole("button", { name: "Allow sound" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("still blocks");
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("goes away on Not now", async () => {
    render(<CastSoundPrompt cast={playing} />);
    await screen.findByRole("button", { name: "Allow sound" });
    await userEvent.click(screen.getByRole("button", { name: "Not now" }));
    expect(screen.queryByRole("button", { name: "Allow sound" })).not.toBeInTheDocument();
    await waitFor(() => expect(mocked.dockKeepOpen).toHaveBeenLastCalledWith(false));
  });
});
