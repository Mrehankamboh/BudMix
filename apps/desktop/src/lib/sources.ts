import type { AudioView, CastStatus, DeviceView, SourceView, VolumeSource } from "./types";

export type SourceStatus = "off" | "waiting" | "connecting" | "playing" | "reconnecting" | "paused" | "error";

/** One row of the sources list: something that plays in the buds. */
export interface SourceRow {
  source: VolumeSource;
  title: string;
  status: SourceStatus;
  detail: string;
}

export const STATUS_LABEL: Record<SourceStatus, string> = {
  off: "Off",
  waiting: "Waiting",
  connecting: "Connecting",
  playing: "Playing",
  reconnecting: "Reconnecting",
  paused: "Paused",
  error: "Error",
};

function budsRow(device: DeviceView | null): SourceRow {
  if (!device) {
    return { source: "master", title: "Buds", status: "off", detail: "No buds paired" };
  }
  const status: SourceStatus =
    device.state === "connected"
      ? "playing"
      : device.reconnectAttempt !== null
        ? "reconnecting"
        : device.state === "connecting"
          ? "connecting"
          : device.lastError
            ? "error"
            : "off";
  return { source: "master", title: `Buds · ${device.name}`, status, detail: device.lastError ?? "Master volume" };
}

function laptopRow(audio: AudioView | null): SourceRow {
  if (!audio) return { source: "laptop", title: "Laptop", status: "waiting", detail: "" };
  if (audio.laptopInBuds) {
    return { source: "laptop", title: "Laptop", status: "playing", detail: "YouTube, Zoom and every other app" };
  }
  return {
    source: "laptop",
    title: "Laptop",
    status: audio.routing ? "waiting" : "off",
    detail: audio.defaultOutput ? `Plays on ${audio.defaultOutput.name}` : "No output",
  };
}

function wifiRow(cast: CastStatus | null): SourceRow {
  const base = { source: "phoneWifi" as const, title: "Phone over Wi-Fi" };
  if (!cast) return { ...base, status: "waiting", detail: "" };
  const phone = cast.phone ?? "Phone";
  switch (cast.phase) {
    case "playing":
      return { ...base, title: `${phone} (Wi-Fi)`, status: "playing", detail: "Casting" };
    case "connecting":
      return { ...base, title: `${phone} (Wi-Fi)`, status: "connecting", detail: "Casting" };
    case "needsApproval":
      return { ...base, title: `${phone} (Wi-Fi)`, status: "waiting", detail: "Waiting for your OK" };
    case "idle":
      break;
  }
  switch (cast.receiver) {
    case "ready":
      return { ...base, status: "waiting", detail: 'Ready: tap Cast → "BudMix" on the phone' };
    case "starting":
      return { ...base, status: "connecting", detail: "Starting the receiver" };
    case "failed":
      return { ...base, status: "reconnecting", detail: cast.message ?? "Retrying" };
    case "unsupported":
      return { ...base, status: "error", detail: cast.message ?? "This PC can't receive Miracast" };
    case "notPrivate":
      return { ...base, status: "error", detail: "Network is Public: receiver off" };
    case "off":
      return { ...base, status: "off", detail: "Phone connection is set to Bluetooth only" };
  }
}

function bluetoothRow(phones: SourceView[], cast: CastStatus | null): SourceRow {
  const base = { source: "phoneBluetooth" as const, title: "Phone over Bluetooth" };
  const on = phones.filter((phone) => phone.enabled);
  const phone = on.find((item) => item.state === "streaming") ?? on[0];
  if (!phone) {
    return {
      ...base,
      status: "off",
      detail: phones.length === 0 ? "No phone paired" : "Turned off for every phone",
    };
  }
  const title = `${phone.name} (Bluetooth)`;
  switch (phone.state) {
    case "streaming":
      return { ...base, title, status: "playing", detail: "Playing through the laptop" };
    case "connecting":
      return { ...base, title, status: "connecting", detail: "" };
    case "waiting":
      return { ...base, title, status: "reconnecting", detail: phone.lastError ?? "Phone away or busy" };
    case "paused":
      return {
        ...base,
        title,
        status: "paused",
        detail: cast?.mode === "wifiOnly" ? "Phone connection is Wi-Fi only" : "Held off while the phone casts",
      };
    case "off":
      return { ...base, title, status: "off", detail: "" };
  }
}

/** Every source in the buds, in the order the list shows them. */
export function sourceRows(
  device: DeviceView | null,
  audio: AudioView | null,
  cast: CastStatus | null,
  phones: SourceView[],
): SourceRow[] {
  return [budsRow(device), laptopRow(audio), wifiRow(cast), bluetoothRow(phones, cast)];
}

/** How many sources play in the buds right now (master excluded). */
export function playingCount(rows: SourceRow[]): number {
  return rows.filter((row) => row.source !== "master" && row.status === "playing").length;
}
