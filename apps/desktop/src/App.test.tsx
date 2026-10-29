import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "./App";
import { api } from "./lib/api";
import type { AppInfo, AudioView, DeviceView, Settings, SourceView } from "./lib/types";

vi.mock("./lib/api", () => ({
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
    audioState: vi.fn(),
    onAudioChanged: vi.fn(),
    playTestTone: vi.fn(),
    setupChecks: vi.fn(),
    runFix: vi.fn(),
    castState: vi.fn(),
    onCastChanged: vi.fn(),
    approvePhone: vi.fn(),
    rejectPhone: vi.fn(),
    forgetPhone: vi.fn(),
    setVolume: vi.fn(),
    showCastVideo: vi.fn(),
  },
}));

const mocked = vi.mocked(api);

const settings: Settings = {
  version: 1,
  autoReconnect: true,
  reconnect: { initialDelayMs: 1000, maxDelayMs: 30000, maxAttempts: 8 },
  connectTimeoutMs: 15000,
  preferredDevice: null,
  toggleHotkey: "Ctrl+Alt+B",
  startWithWindows: false,
  bridgedPhones: [],
  routeLaptopAudio: true,
  savedOutput: null,
  setupDone: true,
  phoneMode: "auto",
  allowedPhones: [],
  volumes: {
    master: { percent: 100, muted: false },
    laptop: { percent: 100, muted: false },
    phoneWifi: { percent: 100, muted: false },
    phoneBluetooth: { percent: 100, muted: false },
  },
  showCastVideo: true,
  phonesSeeded: true,
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

const info: AppInfo = { backend: "demo", version: "0.1.0", mica: false, warnings: [] };

const buds: DeviceView = {
  id: "buds",
  name: "Test Buds",
  profiles: ["stereo"],
  state: "disconnected",
  reconnectAttempt: null,
  lastError: null,
  lastConnected: false,
};

let pushDevices: (devices: DeviceView[]) => void = () => {};
let pushSettings: (settings: Settings) => void = () => {};
let pushSources: (sources: SourceView[]) => void = () => {};

const phone: SourceView = {
  id: "phone",
  name: "Pixel",
  enabled: false,
  state: "off",
  lastError: null,
};

beforeEach(() => {
  mocked.listUnavailablePhones.mockResolvedValue([]);
  mocked.audioState.mockResolvedValue(audio);
  mocked.onAudioChanged.mockResolvedValue(() => {});
  mocked.playTestTone.mockResolvedValue(undefined);
  mocked.setupChecks.mockResolvedValue([]);
  mocked.runFix.mockResolvedValue(undefined);
  mocked.castState.mockResolvedValue(null);
  mocked.onCastChanged.mockResolvedValue(() => {});
  for (const fn of [
    mocked.approvePhone,
    mocked.rejectPhone,
    mocked.forgetPhone,
    mocked.setVolume,
    mocked.showCastVideo,
  ]) {
    fn.mockResolvedValue(undefined);
  }
  mocked.appInfo.mockResolvedValue(info);
  mocked.getSettings.mockResolvedValue(settings);
  mocked.listDevices.mockResolvedValue([buds]);
  mocked.connect.mockResolvedValue(undefined);
  mocked.disconnect.mockResolvedValue(undefined);
  mocked.updateSettings.mockImplementation(async () => info);
  mocked.onDevicesChanged.mockImplementation(async (handler) => {
    pushDevices = handler;
    return () => {};
  });
  mocked.onSettingsChanged.mockImplementation(async (handler) => {
    pushSettings = handler;
    return () => {};
  });
  mocked.listSources.mockResolvedValue([phone]);
  mocked.setSourceEnabled.mockResolvedValue(undefined);
  mocked.onSourcesChanged.mockImplementation(async (handler) => {
    pushSources = handler;
    return () => {};
  });
});

describe("App", () => {
  it("lists devices and flags demo mode", async () => {
    render(<App />);
    expect(await screen.findByText("Test Buds")).toBeInTheDocument();
    expect(screen.getByText("Demo mode: simulated devices")).toBeInTheDocument();
    expect(await screen.findByText("v0.1.0")).toBeInTheDocument();
  });

  it("connects a disconnected device", async () => {
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "Connect Test Buds" }));
    expect(mocked.connect).toHaveBeenCalledWith("buds");
  });

  it("follows live updates from the runtime", async () => {
    render(<App />);
    await screen.findByText("Not connected");
    act(() => pushDevices([{ ...buds, state: "connected" }]));
    const card = screen.getByRole("listitem", { name: "Test Buds" });
    expect(within(card).getByText("Connected")).toBeInTheDocument();
    await userEvent.click(within(card).getByRole("button", { name: "Disconnect Test Buds" }));
    expect(mocked.disconnect).toHaveBeenCalledWith("buds");
  });

  it("shows an empty state", async () => {
    mocked.listDevices.mockResolvedValue([]);
    render(<App />);
    expect(await screen.findByText(/No paired Bluetooth audio devices/)).toBeInTheDocument();
  });

  it("saves settings and shows warnings", async () => {
    mocked.updateSettings.mockResolvedValue({
      ...info,
      warnings: ["Hotkey Ctrl+Alt+B is already used by another app."],
    });
    render(<App />);
    await userEvent.click(await screen.findByRole("checkbox", { name: /Auto-reconnect/ }));
    expect(mocked.updateSettings).toHaveBeenCalledWith({ ...settings, autoReconnect: false });
    expect(await screen.findByText(/already used by another app/)).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: /Auto-reconnect/ })).not.toBeChecked();
  });

  it("turns laptop audio in the buds off", async () => {
    render(<App />);
    await userEvent.click(await screen.findByRole("checkbox", { name: /Play laptop audio in my buds/ }));
    expect(mocked.updateSettings).toHaveBeenCalledWith({ ...settings, routeLaptopAudio: false });
  });

  it("saves the hotkey and the hotkey target", async () => {
    render(<App />);
    const input = await screen.findByLabelText("Toggle hotkey");
    await userEvent.clear(input);
    await userEvent.type(input, "Ctrl+Shift+F9");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(mocked.updateSettings).toHaveBeenLastCalledWith({
      ...settings,
      toggleHotkey: "Ctrl+Shift+F9",
    });

    await userEvent.click(screen.getByRole("radio", { name: "Hotkey target" }));
    expect(mocked.updateSettings).toHaveBeenLastCalledWith(
      expect.objectContaining({ preferredDevice: "buds" }),
    );
  });

  it("follows settings changed from the tray", async () => {
    render(<App />);
    const checkbox = await screen.findByRole("checkbox", { name: /Auto-reconnect/ });
    expect(checkbox).toBeChecked();
    act(() => pushSettings({ ...settings, autoReconnect: false, toggleHotkey: "Ctrl+F12" }));
    expect(checkbox).not.toBeChecked();
    expect(screen.getByLabelText("Toggle hotkey")).toHaveValue("Ctrl+F12");
  });

  it("turns phone audio on and shows its progress", async () => {
    render(<App />);
    await userEvent.click(await screen.findByRole("switch", { name: "Hear Pixel in my buds" }));
    expect(mocked.setSourceEnabled).toHaveBeenCalledWith("phone", true);

    act(() => pushSources([{ ...phone, enabled: true, state: "streaming" }]));
    const card = screen.getByRole("listitem", { name: "Pixel" });
    expect(within(card).getByText("Playing phone audio")).toBeInTheDocument();
    expect(within(card).getByRole("switch")).toBeChecked();
  });

  it("explains how to add a phone when none is paired", async () => {
    mocked.listSources.mockResolvedValue([]);
    render(<App />);
    expect(await screen.findByText(/No phone is paired with this laptop yet/)).toBeInTheDocument();
  });

  it("reports failed requests", async () => {
    mocked.connect.mockRejectedValue("device is not paired");
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "Connect Test Buds" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("device is not paired");
  });
});
