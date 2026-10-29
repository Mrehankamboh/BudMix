import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { CastStatus } from "../../lib/types";
import { CastBanner } from "./CastBanner";

const idle: CastStatus = {
  receiver: "ready",
  message: null,
  phone: null,
  phase: "idle",
  pin: null,
  allowed: [],
  mode: "auto",
  bluetoothSuspended: false,
  videoVisible: true,
  lastError: null,
};

function renderBanner(cast: CastStatus | null) {
  const handlers = { onApprove: vi.fn(), onReject: vi.fn(), onShowVideo: vi.fn() };
  const { container } = render(<CastBanner cast={cast} {...handlers} />);
  return { ...handlers, container };
}

describe("CastBanner", () => {
  it("shows nothing while no phone casts", () => {
    expect(renderBanner(idle).container).toBeEmptyDOMElement();
    expect(renderBanner(null).container).toBeEmptyDOMElement();
  });

  it("asks about a new phone, with its PIN", async () => {
    const { onApprove, onReject } = renderBanner({ ...idle, phone: "Infinix", phase: "needsApproval", pin: "4821" });
    expect(screen.getByText("Infinix wants to play in your buds")).toBeInTheDocument();
    expect(screen.getByText("4821")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Allow always" }));
    expect(onApprove).toHaveBeenCalledWith(true);
    await userEvent.click(screen.getByRole("button", { name: "Just this time" }));
    expect(onApprove).toHaveBeenLastCalledWith(false);
    await userEvent.click(screen.getByRole("button", { name: "Refuse" }));
    expect(onReject).toHaveBeenCalledTimes(1);
  });

  it("hides and shows the phone screen while casting", async () => {
    const { onShowVideo } = renderBanner({ ...idle, phone: "Infinix", phase: "playing" });
    expect(screen.getByText("Infinix is casting over Wi-Fi")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Hide phone screen" }));
    expect(onShowVideo).toHaveBeenCalledWith(false);
  });
});
