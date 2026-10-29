import { useCallback, useEffect, useState } from "react";

import { api } from "./api";
import { isEngaged } from "./format";
import type {
  AppInfo,
  AudioView,
  DeviceView,
  Settings,
  SourceView,
  UnavailablePhone,
} from "./types";
import { useLiveList } from "./useLiveList";

/** Everything both windows show, kept in sync with the Rust runtimes, plus the actions on it. */
export function useBudsLink() {
  const { items: devices, error: loadError } = useLiveList(api.listDevices, api.onDevicesChanged);
  const { items: phones } = useLiveList(api.listSources, api.onSourcesChanged);
  const [unavailablePhones, setUnavailablePhones] = useState<UnavailablePhone[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [audio, setAudio] = useState<AudioView | null>(null);
  const [tone, setTone] = useState<"idle" | "playing">("idle");

  useEffect(() => {
    api.appInfo().then(setInfo).catch((err: unknown) => setActionError(String(err)));
    api.getSettings().then(setSettings).catch((err: unknown) => setActionError(String(err)));
    const unlisten = api.onSettingsChanged(setSettings);
    return () => void unlisten.then((stop) => stop());
  }, []);

  useEffect(() => {
    // Subscribe first so no change is missed between the load and the listener.
    let live = false;
    const unlisten = api.onAudioChanged((next) => {
      live = true;
      setAudio(next);
    });
    api
      .audioState()
      .then((initial) => {
        if (!live) setAudio(initial);
      })
      .catch((err: unknown) => console.warn("reading laptop audio failed", err));
    return () => void unlisten.then((stop) => stop());
  }, []);

  // Pairing changes don't send events, so check again on every phone update
  // and whenever the window is shown.
  useEffect(() => {
    let active = true;
    const load = () => {
      api
        .listUnavailablePhones()
        .then((list) => {
          if (active) setUnavailablePhones(list);
        })
        .catch((err: unknown) => console.warn("listing unavailable phones failed", err));
    };
    load();
    window.addEventListener("focus", load);
    return () => {
      active = false;
      window.removeEventListener("focus", load);
    };
  }, [phones]);

  useEffect(() => {
    document.documentElement.dataset.backdrop = info?.mica ? "mica" : "solid";
  }, [info?.mica]);

  const report = useCallback((err: unknown) => setActionError(String(err)), []);

  const saveSettings = useCallback(
    async (next: Settings) => {
      setActionError(null);
      try {
        setInfo(await api.updateSettings(next));
        setSettings(next);
      } catch (err) {
        report(err);
      }
    },
    [report],
  );

  const toggleDevice = useCallback(
    (device: DeviceView) => {
      setActionError(null);
      const request = isEngaged(device.state) ? api.disconnect(device.id) : api.connect(device.id);
      request.catch(report);
    },
    [report],
  );

  const togglePhone = useCallback(
    (phone: SourceView, enabled: boolean) => {
      setActionError(null);
      api.setSourceEnabled(phone.id, enabled).catch(report);
    },
    [report],
  );

  const playTestTone = useCallback(async () => {
    setActionError(null);
    setTone("playing");
    try {
      await api.playTestTone();
    } catch (err) {
      report(err);
    } finally {
      setTone("idle");
    }
  }, [report]);

  const makeHotkeyTarget = useCallback(
    (id: string) => {
      if (settings) void saveSettings({ ...settings, preferredDevice: id });
    },
    [settings, saveSettings],
  );

  return {
    devices,
    phones,
    unavailablePhones,
    settings,
    info,
    audio,
    tone,
    error: actionError ?? loadError,
    saveSettings,
    toggleDevice,
    togglePhone,
    makeHotkeyTarget,
    playTestTone,
  };
}
