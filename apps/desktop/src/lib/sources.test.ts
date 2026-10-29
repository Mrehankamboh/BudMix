import { describe, expect, it } from "vitest";

import { playingCount, sourceRows } from "./sources";
import type { AudioView, CastStatus, DeviceView, SourceView } from "./types";

const buds: DeviceView = {
  id: "tws",
  name: "TWS",
  profiles: ["stereo"],
  state: "connected",
  reconnectAttempt: null,
  lastError: null,
  lastConnected: true,
};

const audio: AudioView = {
  routing: true,
  buds: "tws",
  budsOutput: { id: "e1", name: "Headphones (TWS)" },
  defaultOutput: { id: "e1", name: "Headphones (TWS)" },
  laptopInBuds: true,
  callMode: false,
  savedOutput: null,
  lastError: null,
};

const cast: CastStatus = {
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

const infinix: SourceView = { id: "p", name: "Infinix", enabled: true, state: "streaming", lastError: null };

describe("sourceRows", () => {
  it("lists buds, laptop, Wi-Fi and Bluetooth with their state", () => {
    const rows = sourceRows(buds, audio, cast, [infinix]);
    expect(rows.map((row) => [row.source, row.status])).toEqual([
      ["master", "playing"],
      ["laptop", "playing"],
      ["phoneWifi", "waiting"],
      ["phoneBluetooth", "playing"],
    ]);
    expect(playingCount(rows)).toBe(2);
  });

  it("shows the casting phone and Bluetooth held off", () => {
    const casting: CastStatus = { ...cast, phone: "Infinix", phase: "playing", bluetoothSuspended: true };
    const rows = sourceRows(buds, audio, casting, [{ ...infinix, state: "paused" }]);
    expect(rows[2]).toMatchObject({ title: "Infinix (Wi-Fi)", status: "playing" });
    expect(rows[3]).toMatchObject({ status: "paused", detail: "Held off while the phone casts" });
    expect(playingCount(rows)).toBe(2);
  });

  it("explains why the receiver is off", () => {
    expect(sourceRows(buds, audio, { ...cast, receiver: "notPrivate" }, [])[2]?.detail).toMatch(/Public/);
    expect(sourceRows(buds, audio, { ...cast, receiver: "off", mode: "bluetoothOnly" }, [])[2]?.status).toBe("off");
    expect(
      sourceRows(buds, audio, { ...cast, receiver: "unsupported", message: "No Miracast here" }, [])[2],
    ).toMatchObject({ status: "error", detail: "No Miracast here" });
  });

  it("shows buds reconnecting and laptop audio elsewhere", () => {
    const rows = sourceRows(
      { ...buds, state: "disconnected", reconnectAttempt: 2 },
      { ...audio, laptopInBuds: false, defaultOutput: { id: "s", name: "Speakers" } },
      cast,
      [],
    );
    expect(rows[0]?.status).toBe("reconnecting");
    expect(rows[1]).toMatchObject({ status: "waiting", detail: "Plays on Speakers" });
    expect(rows[3]).toMatchObject({ status: "off", detail: "No phone paired" });
  });

  it("waits for a phone that needs approval", () => {
    const rows = sourceRows(buds, audio, { ...cast, phone: "Guest", phase: "needsApproval" }, []);
    expect(rows[2]).toMatchObject({ status: "waiting", detail: "Waiting for your OK" });
  });
});
