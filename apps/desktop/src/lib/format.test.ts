import { describe, expect, it } from "vitest";

import { bridgeStatusLabel, isEngaged, profileLabel, shortName, statusLabel } from "./format";
import type { DeviceView } from "./types";

const base: DeviceView = {
  id: "buds",
  name: "Buds",
  profiles: ["stereo"],
  state: "disconnected",
  reconnectAttempt: null,
  lastError: null,
  lastConnected: false,
};

describe("statusLabel", () => {
  it("describes plain states", () => {
    expect(statusLabel({ ...base, state: "connected" })).toBe("Connected");
    expect(statusLabel({ ...base, state: "connecting" })).toBe("Connecting…");
    expect(statusLabel(base)).toBe("Not connected");
    expect(statusLabel({ ...base, state: "disconnecting" })).toBe("Disconnecting…");
  });

  it("describes auto-reconnect progress", () => {
    expect(statusLabel({ ...base, state: "connecting", reconnectAttempt: 2 })).toBe(
      "Reconnecting (attempt 2)…",
    );
    expect(statusLabel({ ...base, reconnectAttempt: 3 })).toBe("Reconnect attempt 3 scheduled");
  });
});

describe("helpers", () => {
  it("labels profiles", () => {
    expect(profileLabel("handsFree")).toBe("Hands-free");
    expect(profileLabel("leAudio")).toBe("LE Audio");
  });

  it("treats connecting as engaged", () => {
    expect(isEngaged("connected")).toBe(true);
    expect(isEngaged("connecting")).toBe(true);
    expect(isEngaged("disconnecting")).toBe(false);
    expect(isEngaged("disconnected")).toBe(false);
  });
});

describe("bridgeStatusLabel", () => {
  it("describes every bridge state", () => {
    expect(bridgeStatusLabel("off")).toBe("Off");
    expect(bridgeStatusLabel("connecting")).toBe("Connecting to the phone…");
    expect(bridgeStatusLabel("streaming")).toBe("Playing phone audio");
    expect(bridgeStatusLabel("waiting")).toBe("Waiting for the phone (retrying)");
  });
});

describe("shortName", () => {
  it("keeps short names and trims long ones", () => {
    expect(shortName("TWS")).toBe("TWS");
    expect(shortName("Demo Headphones")).toBe("Demo Headpho…");
    expect(shortName("Demo Headphones", 20)).toBe("Demo Headphones");
  });
});
