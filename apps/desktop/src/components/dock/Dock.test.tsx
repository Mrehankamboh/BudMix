import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { api } from "../../lib/api";
import type { AppInfo, AudioView, DeviceView, Settings, SourceView } from "../../lib/types";
import { Dock } from "./Dock";

vi.mock("../../lib/api", () => ({
  api: {
    appInfo: vi.fn(),
    listDevices: vi.fn(),
    connect: vi.fn(),
    disconnect: vi.fn(),
    getSettings: vi.fn(),
    updateSettings: vi.fn(),
    onDevicesChanged: vi.fn(),
    onSettingsChanged: vi.fn(),
    listSources: vi.fn(),
    setSourceEnabled: vi.fn(),
    onSourcesChanged: vi.fn(),
    listUnavailablePhones: vi.fn(),
    diagnostics: vi.fn(),
    openSettings: vi.fn(),
    quitApp: vi.fn(),
    dockResize: vi.fn(),
    hideDock: vi.fn(),
    audioState: vi.fn(),
    onAudioChanged: vi.fn(),
    playTestTone: vi.fn(),
    setupChecks: vi.fn(),
    runFix: vi.fn(),
  },
}));

const mocked = vi.mocked(api);

const settings: Settings = {
  version: 1,
  autoReconnect: true,
  reconnect: { initialDelayMs: 1000, maxDelayMs: 30000, maxAttempts: 8 },
  connectTimeoutMs: 15000,
  preferredDevice: "tws",
  toggleHotkey: "Ctrl+Alt+B",
  startWithWindows: false,
  bridgedPhones: [],
  routeLaptopAudio: true,
  savedOutput: null,
  setupDone: true,
};

const audio: AudioView = {
  routing: true,
  buds: "tws",
  budsOutput: { id: "e-tws", name: "Headphones (TWS)" },
  defaultOutput: { id: "e-tws", name: "Headphones (TWS)" },
  laptopInBuds: true,
  callMode: false,
  savedOutput: null,
  lastError: null,
};

const tws: DeviceView = {
  id: "tws",
  name: "TWS",
  profiles: ["stereo"],
  state: "connected",
  reconnectAttempt: null,
  lastError: null,
  lastConnected: false,
};

const speaker: DeviceView = { ...tws, id: "speaker", name: "Speaker", state: "disconnected" };

const pixel: SourceView = { id: "pixel", name: "Pixel 7", enabled: false, state: "off", lastError: null };

let pushSources: (sources: SourceView[]) => void = () => {};

beforeEach(() => {
  mocked.appInfo.mockResolvedValue({ backend: "windows", version: "0.3.0", mica: false, warnings: [] } satisfies AppInfo);
  mocked.getSettings.mockResolvedValue(settings);
  mocked.listDevices.mockResolvedValue([tws, speaker]);
  mocked.listSources.mockResolvedValue([pixel]);
  mocked.listUnavailablePhones.mockResolvedValue([]);
  mocked.audioState.mockResolvedValue(audio);
  mocked.onAudioChanged.mockResolvedValue(() => {});
  mocked.playTestTone.mockResolvedValue(undefined);
  mocked.setupChecks.mockResolvedValue([]);
  mocked.runFix.mockResolvedValue(undefined);
  for (const fn of [
    mocked.connect,
    mocked.disconnect,
    mocked.setSourceEnabled,
    mocked.openSettings,
    mocked.quitApp,
    mocked.dockResize,
  ]) {
    fn.mockResolvedValue(undefined);
  }
  mocked.updateSettings.mockResolvedValue({ backend: "windows", version: "0.3.0", mica: false, warnings: [] });
  mocked.onDevicesChanged.mockResolvedValue(() => {});
  mocked.onSettingsChanged.mockResolvedValue(() => {});
  mocked.onSourcesChanged.mockImplementation(async (handler) => {
    pushSources = handler;
    return () => {};
  });
});

describe("Dock", () => {
  it("summarises the buds, the phone and the sound sources", async () => {
    render(<Dock />);
    expect(await screen.findByText("Buds on this laptop")).toBeInTheDocument();
    expect(screen.getByText("TWS connected · phone audio off")).toBeInTheDocument();
    expect(screen.getByText("1")).toBeInTheDocument();
    expect(screen.getByText("source")).toBeInTheDocument();
    expect(screen.getByRole("img")).toHaveAccessibleName(/Pixel 7: off\. TWS: connected/);
  });

  it("shows phone audio playing as two sources", async () => {
    render(<Dock />);
    await screen.findByText("Buds on this laptop");
    act(() => pushSources([{ ...pixel, enabled: true, state: "streaming" }]));
    expect(screen.getByText("Hearing phone + laptop")).toBeInTheDocument();
    expect(screen.getByText("2")).toBeInTheDocument();
    expect(screen.getByText("phone + laptop")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Phone audio: Playing phone audio/ })).toBeInTheDocument();
  });

  it("shows every phone that plays, and counts them", async () => {
    render(<Dock />);
    await screen.findByText("Buds on this laptop");
    const oppo: SourceView = { ...pixel, id: "oppo", name: "OPPO A83" };
    act(() =>
      pushSources([
        { ...oppo, enabled: true, state: "streaming" },
        { ...pixel, enabled: true, state: "streaming" },
      ]),
    );
    expect(screen.getByText("Hearing 2 phones + laptop")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
    expect(screen.getByText("2 phones + laptop")).toBeInTheDocument();
    expect(screen.getByText("2 phones")).toBeInTheDocument();
    expect(screen.getByRole("img")).toHaveAccessibleName(
      /OPPO A83: playing phone audio\. Pixel 7: playing phone audio\. TWS: connected/,
    );
  });

  it("controls everything from the tiles", async () => {
    render(<Dock />);
    await userEvent.click(await screen.findByRole("button", { name: "Buds: Connected" }));
    expect(mocked.disconnect).toHaveBeenCalledWith("tws");

    await userEvent.click(screen.getByRole("button", { name: "Phone audio: Off" }));
    expect(mocked.setSourceEnabled).toHaveBeenCalledWith("pixel", true);

    await userEvent.click(screen.getByRole("button", { name: "Auto-reconnect on" }));
    expect(mocked.updateSettings).toHaveBeenCalledWith({ ...settings, autoReconnect: false });

    await userEvent.click(screen.getByRole("button", { name: /Hotkey: Ctrl\+Alt\+B/ }));
    expect(mocked.openSettings).toHaveBeenCalled();
  });

  it("toggles the buds with the power button", async () => {
    render(<Dock />);
    await userEvent.click(await screen.findByRole("button", { name: "Disconnect TWS" }));
    expect(mocked.disconnect).toHaveBeenCalledWith("tws");
  });

  it("opens the device and phone lists in place", async () => {
    render(<Dock />);
    const devices = await screen.findByRole("button", { name: /Devices/ });
    expect(devices).toHaveAttribute("aria-expanded", "false");
    await userEvent.click(devices);
    expect(devices).toHaveAttribute("aria-expanded", "true");
    const row = screen.getByRole("listitem", { name: "Speaker" });
    await userEvent.click(within(row).getByRole("button", { name: "Connect Speaker" }));
    expect(mocked.connect).toHaveBeenCalledWith("speaker");

    await userEvent.click(screen.getByRole("button", { name: /Phones/ }));
    expect(devices).toHaveAttribute("aria-expanded", "false");
    await userEvent.click(screen.getByRole("switch", { name: "Hear Pixel 7 in my buds" }));
    expect(mocked.setSourceEnabled).toHaveBeenCalledWith("pixel", true);
  });

  it("opens settings, starts with Windows and quits from the menu", async () => {
    render(<Dock />);
    await userEvent.click(await screen.findByRole("button", { name: "All settings" }));
    await userEvent.click(screen.getByRole("button", { name: "Open settings" }));
    expect(mocked.openSettings).toHaveBeenCalledTimes(2);

    await userEvent.click(screen.getByRole("button", { name: "More" }));
    await userEvent.click(screen.getByRole("menuitemcheckbox", { name: "Start with Windows" }));
    expect(mocked.updateSettings).toHaveBeenCalledWith({ ...settings, startWithWindows: true });

    await userEvent.click(screen.getByRole("button", { name: "More" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Quit BudsLink" }));
    expect(mocked.quitApp).toHaveBeenCalled();
  });

  it("explains what to do without paired devices", async () => {
    mocked.listDevices.mockResolvedValue([]);
    mocked.listSources.mockResolvedValue([]);
    render(<Dock />);
    expect(await screen.findByText("No buds paired")).toBeInTheDocument();
    expect(screen.getByText("0")).toBeInTheDocument();
  });

  it("lists a paired phone Windows won't stream from, with what to do", async () => {
    mocked.listSources.mockResolvedValue([]);
    mocked.listUnavailablePhones.mockResolvedValue([{ name: "Pixel 7", reason: "noAudioService" }]);
    render(<Dock />);
    const tile = await screen.findByRole("button", { name: "Phone audio unavailable for Pixel 7. Show why" });
    expect(screen.getByRole("button", { name: /Phones/ })).toHaveTextContent("1 paired");

    await userEvent.click(tile);
    const row = screen.getByRole("listitem", { name: "Pixel 7" });
    expect(row).toHaveTextContent("Windows doesn't offer its audio");
    expect(within(row).queryByRole("switch")).not.toBeInTheDocument();
    expect(screen.getByText(/pair it again from the laptop/)).toBeInTheDocument();
  });

  it("shows the latest problem as a banner", async () => {
    mocked.listDevices.mockResolvedValue([{ ...tws, state: "disconnected", lastError: "Connection timed out." }]);
    render(<Dock />);
    expect(await screen.findByRole("alert")).toHaveTextContent("Connection timed out.");
  });

  it("says where laptop audio plays", async () => {
    render(<Dock />);
    expect(await screen.findByText("Laptop audio plays in your buds")).toBeInTheDocument();
  });

  it("names the other output when laptop audio is not in the buds", async () => {
    mocked.audioState.mockResolvedValue({
      ...audio,
      laptopInBuds: false,
      defaultOutput: { id: "e-spk", name: "Speakers (Realtek)" },
    });
    render(<Dock />);
    expect(await screen.findByText("Laptop audio plays on Speakers (Realtek)")).toBeInTheDocument();
  });

  it("plays the test beep and shows it is busy meanwhile", async () => {
    let finish: () => void = () => {};
    mocked.playTestTone.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    );
    render(<Dock />);
    await userEvent.click(await screen.findByRole("button", { name: "Test beep" }));
    expect(mocked.playTestTone).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button", { name: "Playing…" })).toBeDisabled();
    await act(async () => finish());
    expect(screen.getByRole("button", { name: "Test beep" })).toBeEnabled();
  });

  it("warns when the buds switch to call mode", async () => {
    mocked.audioState.mockResolvedValue({ ...audio, callMode: true });
    render(<Dock />);
    expect(await screen.findByRole("alert")).toHaveTextContent(/call mode \(HFP\)/);
  });

  it("follows laptop audio changes pushed by the app", async () => {
    let pushAudio: (next: AudioView) => void = () => {};
    mocked.onAudioChanged.mockImplementation(async (handler) => {
      pushAudio = handler;
      return () => {};
    });
    render(<Dock />);
    await screen.findByText("Laptop audio plays in your buds");
    act(() => pushAudio({ ...audio, laptopInBuds: false, budsOutput: null, buds: null }));
    expect(screen.getByText("Laptop audio moves to your buds once they connect")).toBeInTheDocument();
  });
});
