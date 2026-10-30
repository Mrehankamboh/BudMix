import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { api } from "../lib/api";
import type { CheckResult } from "../lib/types";
import { SetupPanel, fixLabel } from "./SetupPanel";

vi.mock("../lib/api", () => ({
  api: {
    setupChecks: vi.fn(),
    runFix: vi.fn(),
  },
}));

const mocked = vi.mocked(api);

const publicNetwork: CheckResult = {
  id: "networkPrivate",
  status: "problem",
  title: "Network is Public",
  detail: '"Home Wi-Fi" is set to Public.',
  fix: { kind: "makeNetworkPrivate", network: "Home Wi-Fi" },
};

const miracast: CheckResult = {
  id: "miracast",
  status: "ok",
  title: "Wi-Fi card can receive Miracast",
  detail: "",
  fix: null,
};

beforeEach(() => {
  mocked.setupChecks.mockResolvedValue([publicNetwork, miracast]);
  mocked.runFix.mockResolvedValue(undefined);
});

function renderPanel(done = false) {
  const onDone = vi.fn();
  const onPlayTone = vi.fn().mockResolvedValue(undefined);
  render(<SetupPanel done={done} onDone={onDone} onPlayTone={onPlayTone} />);
  return { onDone, onPlayTone };
}

describe("SetupPanel", () => {
  it("lists every check with its fix", async () => {
    renderPanel();
    const network = await screen.findByRole("listitem", { name: "Network is Public" });
    expect(within(network).getByRole("button", { name: "Make Private (admin)" })).toBeInTheDocument();
    const wifi = screen.getByRole("listitem", { name: "Wi-Fi card can receive Miracast" });
    expect(within(wifi).queryByRole("button")).not.toBeInTheDocument();
  });

  it("runs an admin fix and checks again", async () => {
    renderPanel();
    await userEvent.click(await screen.findByRole("button", { name: "Make Private (admin)" }));
    expect(mocked.runFix).toHaveBeenCalledWith({ kind: "makeNetworkPrivate", network: "Home Wi-Fi" });
    expect(mocked.setupChecks).toHaveBeenCalledTimes(2);
  });

  it("shows why a fix failed", async () => {
    mocked.runFix.mockRejectedValue("The admin prompt was cancelled, so nothing changed.");
    renderPanel();
    await userEvent.click(await screen.findByRole("button", { name: "Make Private (admin)" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("admin prompt was cancelled");
  });

  it("plays the beep and records the answer", async () => {
    const { onPlayTone } = renderPanel();
    const beep = await screen.findByRole("listitem", { name: "Test beep" });
    await userEvent.click(within(beep).getByRole("button", { name: "Play beep" }));
    expect(onPlayTone).toHaveBeenCalledTimes(1);
    await userEvent.click(within(beep).getByRole("button", { name: "No" }));
    expect(beep).toHaveTextContent("Check that the buds are connected");
    await userEvent.click(within(beep).getByRole("button", { name: "Yes" }));
    expect(beep).toHaveTextContent("Beep heard in the buds");
  });

  it("can be finished later while problems remain", async () => {
    const { onDone } = renderPanel();
    await userEvent.click(await screen.findByRole("button", { name: "Finish later" }));
    expect(onDone).toHaveBeenCalledTimes(1);
  });

  it("is a plain re-runnable check once setup is done", async () => {
    renderPanel(true);
    expect(await screen.findByRole("heading", { name: "Setup check" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Done|Finish later/ })).not.toBeInTheDocument();
  });

  it("labels every kind of fix", () => {
    expect(fixLabel({ kind: "openSettings", uri: "ms-settings:bluetooth" })).toBe("Open settings");
    expect(fixLabel({ kind: "addFirewallRule" })).toBe("Allow (admin)");
    expect(fixLabel({ kind: "installWirelessDisplay" })).toBe("Install (admin)");
  });
});
