import { isEngaged } from "./format";
import type { DeviceView, SourceView } from "./types";

/**
 * The buds the dock's main controls act on: the hotkey target, else the
 * connected one, else the one connected most recently, else the first.
 * Mirrors `Controller::hotkey_target`.
 */
export function focusDevice(devices: DeviceView[], preferred: string | null): DeviceView | null {
  return (
    devices.find((device) => device.id === preferred) ??
    devices.find((device) => isEngaged(device.state)) ??
    devices.find((device) => device.lastConnected) ??
    devices[0] ??
    null
  );
}

/** The phone the dock shows: one that is switched on, else the first. */
export function focusPhone(phones: SourceView[]): SourceView | null {
  return phones.find((phone) => phone.enabled) ?? phones[0] ?? null;
}

/** Sound sources reaching the buds: the laptop, plus every phone playing through it. */
export function soundSources(device: DeviceView | null, phones: SourceView[]): number {
  if (device?.state !== "connected") return 0;
  return 1 + phones.filter((phone) => phone.state === "streaming").length;
}

/** What the sources are, shown next to the count. */
export function sourcesLabel(sources: number): string {
  if (sources <= 0) return "buds not here";
  if (sources === 1) return "laptop";
  if (sources === 2) return "phone + laptop";
  return `${sources - 1} phones + laptop`;
}

export interface Headline {
  title: string;
  detail: string;
}

export function headline(device: DeviceView | null, phones: SourceView[]): Headline {
  if (!device) {
    return {
      title: "No buds paired",
      detail: "Pair them in Windows Settings → Bluetooth & devices",
    };
  }
  switch (device.state) {
    case "connecting":
      return { title: `Connecting ${device.name}…`, detail: "Bringing the buds to this laptop" };
    case "disconnecting":
      return { title: `Releasing ${device.name}…`, detail: "Handing the buds back" };
    case "disconnected":
      return {
        title: "Buds not on this laptop",
        detail:
          device.reconnectAttempt !== null
            ? `Reconnecting ${device.name} (attempt ${device.reconnectAttempt})`
            : `${device.name} is free for your phone`,
      };
    case "connected":
      break;
  }
  const playing = phones.filter((item) => item.state === "streaming");
  if (playing.length > 0) {
    return {
      title:
        playing.length === 1 ? "Hearing phone + laptop" : `Hearing ${playing.length} phones + laptop`,
      detail: `${device.name} on this laptop · ${playing.map((item) => item.name).join(" + ")} playing through it`,
    };
  }
  const phone = focusPhone(phones);
  if (phone && (phone.state === "connecting" || phone.state === "waiting")) {
    return { title: "Buds on this laptop", detail: `${device.name} connected · waiting for ${phone.name}` };
  }
  return { title: "Buds on this laptop", detail: `${device.name} connected · phone audio off` };
}
