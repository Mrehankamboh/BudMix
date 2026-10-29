import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { SourceRow } from "../lib/sources";
import type { Volumes } from "../lib/types";
import { SourcesPanel } from "./SourcesPanel";

const full = { percent: 100, muted: false };
const volumes: Volumes = { master: full, laptop: full, phoneWifi: full, phoneBluetooth: full };

const rows: SourceRow[] = [
  { source: "master", title: "Buds · TWS", status: "playing", detail: "Master volume" },
  { source: "laptop", title: "Laptop", status: "playing", detail: "" },
  { source: "phoneWifi", title: "Infinix (Wi-Fi)", status: "playing", detail: "Casting" },
  { source: "phoneBluetooth", title: "Infinix (Bluetooth)", status: "paused", detail: "Held off while the phone casts" },
];

afterEach(() => {
  vi.useRealTimers();
});

describe("SourcesPanel", () => {
  it("lists every source with its status", () => {
    render(<SourcesPanel rows={rows} volumes={volumes} onVolume={vi.fn()} allowed={[]} onForget={vi.fn()} />);
    expect(screen.getByRole("listitem", { name: "Infinix (Bluetooth)" })).toHaveTextContent(
      "Paused · Held off while the phone casts",
    );
    expect(screen.getByRole("listitem", { name: "Laptop" })).toHaveTextContent("Playing");
  });

  it("mutes a source", async () => {
    const onVolume = vi.fn();
    render(<SourcesPanel rows={rows} volumes={volumes} onVolume={onVolume} allowed={[]} onForget={vi.fn()} />);
    await userEvent.click(screen.getByRole("button", { name: "Mute Laptop" }));
    expect(onVolume).toHaveBeenCalledWith("laptop", { percent: 100, muted: true });
  });

  it("sends a slider move after the drag pauses", () => {
    vi.useFakeTimers();
    const onVolume = vi.fn();
    render(<SourcesPanel rows={rows} volumes={volumes} onVolume={onVolume} allowed={[]} onForget={vi.fn()} />);
    const slider = screen.getByRole("slider", { name: "Infinix (Wi-Fi) volume" });
    fireEvent.change(slider, { target: { value: "40" } });
    fireEvent.change(slider, { target: { value: "35" } });
    expect(onVolume).not.toHaveBeenCalled();
    expect(screen.getByText("35%")).toBeInTheDocument();
    act(() => vi.advanceTimersByTime(250));
    expect(onVolume).toHaveBeenCalledTimes(1);
    expect(onVolume).toHaveBeenCalledWith("phoneWifi", { percent: 35, muted: false });
  });

  it("disconnects the casting phone and forgets an allowed one", async () => {
    const onDisconnectCast = vi.fn();
    const onForget = vi.fn();
    render(
      <SourcesPanel
        rows={rows}
        volumes={volumes}
        onVolume={vi.fn()}
        onDisconnectCast={onDisconnectCast}
        allowed={["Infinix"]}
        onForget={onForget}
      />,
    );
    const wifi = screen.getByRole("listitem", { name: "Infinix (Wi-Fi)" });
    await userEvent.click(within(wifi).getByRole("button", { name: "Disconnect" }));
    expect(onDisconnectCast).toHaveBeenCalledTimes(1);
    await userEvent.click(within(screen.getByRole("listitem", { name: "Infinix" })).getByRole("button", { name: "Forget" }));
    expect(onForget).toHaveBeenCalledWith("Infinix");
  });

  it("leaves out the heading and allowlist inside the dock", () => {
    render(<SourcesPanel compact rows={rows} volumes={volumes} onVolume={vi.fn()} allowed={["Infinix"]} onForget={vi.fn()} />);
    expect(screen.queryByRole("heading")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Forget" })).not.toBeInTheDocument();
  });
});
