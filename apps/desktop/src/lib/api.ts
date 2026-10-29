import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  AppInfo,
  AudioView,
  CheckResult,
  DeviceView,
  Fix,
  Settings,
  SourceView,
  UnavailablePhone,
} from "./types";

export const DEVICES_CHANGED = "devices-changed";
export const SETTINGS_CHANGED = "settings-changed";
export const SOURCES_CHANGED = "sources-changed";
export const AUDIO_CHANGED = "audio-changed";

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  listDevices: () => invoke<DeviceView[]>("list_devices"),
  connect: (id: string) => invoke<void>("connect_device", { id }),
  disconnect: (id: string) => invoke<void>("disconnect_device", { id }),
  getSettings: () => invoke<Settings>("get_settings"),
  /** Saves settings and returns the updated app info (with any warnings). */
  updateSettings: (settings: Settings) => invoke<AppInfo>("update_settings", { settings }),
  onDevicesChanged: (handler: (devices: DeviceView[]) => void): Promise<UnlistenFn> =>
    listen<DeviceView[]>(DEVICES_CHANGED, (event) => handler(event.payload)),
  listSources: () => invoke<SourceView[]>("list_sources"),
  /** Turns bridge mode on or off for a phone. */
  setSourceEnabled: (id: string, enabled: boolean) =>
    invoke<void>("set_source_enabled", { id, enabled }),
  onSourcesChanged: (handler: (sources: SourceView[]) => void): Promise<UnlistenFn> =>
    listen<SourceView[]>(SOURCES_CHANGED, (event) => handler(event.payload)),
  /** Paired phones Windows won't stream audio from. */
  listUnavailablePhones: () => invoke<UnavailablePhone[]>("list_unavailable_phones"),
  /** Laptop audio: default output, buds output, call mode. */
  audioState: () => invoke<AudioView>("audio_state"),
  onAudioChanged: (handler: (audio: AudioView) => void): Promise<UnlistenFn> =>
    listen<AudioView>(AUDIO_CHANGED, (event) => handler(event.payload)),
  /** Plays the test beep into the buds; resolves once it has played. */
  playTestTone: () => invoke<void>("play_test_tone"),
  /** Every prerequisite with its status and fix (takes a second or two). */
  setupChecks: () => invoke<CheckResult[]>("setup_checks"),
  /** Runs the fix behind a check's button; admin fixes show one Windows prompt. */
  runFix: (fix: Fix) => invoke<void>("run_fix", { fix }),
  /** What BudsLink and Windows see, as text for a bug report. */
  diagnostics: () => invoke<string>("diagnostics"),
  /** Opens the full window (settings) and closes the dock. */
  openSettings: () => invoke<void>("open_settings"),
  quitApp: () => invoke<void>("quit_app"),
  /** Tells the dock window how tall its content is. */
  dockResize: (height: number) => invoke<void>("dock_resize", { height }),
  hideDock: () => invoke<void>("hide_dock"),
  /** Fires when settings change elsewhere, e.g. from the tray menu. */
  onSettingsChanged: (handler: (settings: Settings) => void): Promise<UnlistenFn> =>
    listen<Settings>(SETTINGS_CHANGED, (event) => handler(event.payload)),
};
