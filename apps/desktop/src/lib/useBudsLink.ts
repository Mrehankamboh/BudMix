import { useCallback, useEffect, useState } from "react";

import { api } from "./api";
import { isEngaged } from "./format";
import type { UnlistenFn } from "@tauri-apps/api/event";

import type {
  AppInfo,
  AudioView,
  CastStatus,
  DeviceView,
  Level,
  Settings,
  SourceView,
  UnavailablePhone,
  VolumeSource,
} from "./types";
import { useLiveList } from "./useLiveList";

/**
 * One value kept in sync with a Rust runtime. Subscribes first, so no change
 * is missed between the load and the listener. `load` and `subscribe` must
 * be stable.
 */
function useLiveValue<T>(
  load: () => Promise<T | null>,
  subscribe: (handler: (value: T) => void) => Promise<UnlistenFn>,
): T | null {
  const [value, setValue] = useState<T | null>(null);
  useEffect(() => {
    let live = false;
    const unlisten = subscribe((next) => {
      live = true;
      setValue(next);
    });
    load()
      .then((initial) => {
        if (!live) setValue(initial);
      })
      .catch((err: unknown) => console.warn("loading state failed", err));
    return () => void unlisten.then((stop) => stop());
  }, [load, subscribe]);
  return value;
}

/** Everything both windows show, kept in sync with the Rust runtimes, plus the actions on it. */
export function useBudsLink() {
  const { items: devices, error: loadError } = useLiveList(api.listDevices, api.onDevicesChanged);
  const { items: phones } = useLiveList(api.listSources, api.onSourcesChanged);
  const [unavailablePhones, setUnavailablePhones] = useState<UnavailablePhone[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const audio = useLiveValue<AudioView>(api.audioState, api.onAudioChanged);
  const cast = useLiveValue<CastStatus>(api.castState, api.onCastChanged);
  const [tone, setTone] = useState<"idle" | "playing">("idle");

  useEffect(() => {
    api.appInfo().then(setInfo).catch((err: unknown) => setActionError(String(err)));
    api.getSettings().then(setSettings).catch((err: unknown) => setActionError(String(err)));
    const unlisten = api.onSettingsChanged(setSettings);
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

  const run = useCallback(
    (request: Promise<void>) => {
      setActionError(null);
      request.catch(report);
    },
    [report],
  );

  const approvePhone = useCallback((remember: boolean) => run(api.approvePhone(remember)), [run]);
  const rejectPhone = useCallback(() => run(api.rejectPhone()), [run]);
  const forgetPhone = useCallback((name: string) => run(api.forgetPhone(name)), [run]);
  const setVolume = useCallback(
    (source: VolumeSource, level: Level) => run(api.setVolume(source, level)),
    [run],
  );
  const showCastVideo = useCallback((visible: boolean) => run(api.showCastVideo(visible)), [run]);

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
    cast,
    tone,
    error: actionError ?? loadError,
    saveSettings,
    toggleDevice,
    togglePhone,
    makeHotkeyTarget,
    playTestTone,
    approvePhone,
    rejectPhone,
    forgetPhone,
    setVolume,
    showCastVideo,
  };
}
