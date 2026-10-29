import { describe, expect, it } from "vitest";

import { focusDevice, focusPhone, headline, soundSources, sourcesLabel } from "./focus";
import type { DeviceView, SourceView } from "./types";

function device(id: string, state: DeviceView["state"] = "disconnected"): DeviceView {
  return {
    id,
    name: id.toUpperCase(),
    profiles: ["stereo"],
    state,
    reconnectAttempt: null,
    lastError: null,
    lastConnected: false,
  };
}

function phone(id: string, state: SourceView["state"] = "off", enabled = false): SourceView {
  return { id, name: id.toUpperCase(), enabled, state, lastError: null };
}

describe("focusDevice", () => {
  it("prefers the hotkey target, then the connected device, then the first", () => {
    const list = [device("a"), device("b", "connected")];
    expect(focusDevice(list, "a")?.id).toBe("a");
    expect(focusDevice(list, "missing")?.id).toBe("b");
    expect(focusDevice(list, null)?.id).toBe("b");
    expect(focusDevice([device("a"), device("b")], null)?.id).toBe("a");
    expect(focusDevice([], null)).toBeNull();
  });

  it("keeps the buds just released in focus", () => {
    const released = { ...device("b"), lastConnected: true };
    expect(focusDevice([device("a"), released], null)?.id).toBe("b");
    expect(focusDevice([device("a", "connected"), released], null)?.id).toBe("a");
  });
});

describe("focusPhone", () => {
  it("prefers a phone that is switched on", () => {
    expect(focusPhone([phone("a"), phone("b", "waiting", true)])?.id).toBe("b");
    expect(focusPhone([phone("a"), phone("b")])?.id).toBe("a");
    expect(focusPhone([])).toBeNull();
  });
});

describe("soundSources", () => {
  it("counts the laptop and every streaming phone, only while the buds are connected", () => {
    expect(soundSources(null, [])).toBe(0);
    expect(soundSources(device("a"), [phone("p", "streaming", true)])).toBe(0);
    expect(soundSources(device("a", "connected"), [])).toBe(1);
    expect(soundSources(device("a", "connected"), [phone("p", "streaming", true)])).toBe(2);
  });
});

describe("headline", () => {
  it("covers every situation", () => {
    expect(headline(null, []).title).toBe("No buds paired");
    expect(headline(device("a", "connecting"), []).title).toBe("Connecting A…");
    expect(headline(device("a", "disconnecting"), []).title).toBe("Releasing A…");
    expect(headline(device("a"), [])).toEqual({
      title: "Buds not on this laptop",
      detail: "A is free for your phone",
    });
    expect(headline({ ...device("a"), reconnectAttempt: 2 }, []).detail).toBe(
      "Reconnecting A (attempt 2)",
    );
    expect(headline(device("a", "connected"), [phone("p", "streaming", true)])).toEqual({
      title: "Hearing phone + laptop",
      detail: "A on this laptop · P playing through it",
    });
    expect(headline(device("a", "connected"), [phone("p", "waiting", true)]).detail).toBe(
      "A connected · waiting for P",
    );
    expect(headline(device("a", "connected"), [phone("p")]).detail).toBe(
      "A connected · phone audio off",
    );
    expect(
      headline(device("a", "connected"), [phone("p", "streaming", true), phone("q", "streaming", true)]),
    ).toEqual({
      title: "Hearing 2 phones + laptop",
      detail: "A on this laptop · P + Q playing through it",
    });
  });
});

describe("sourcesLabel", () => {
  it("names what reaches the buds, for any number of phones", () => {
    expect(sourcesLabel(0)).toBe("buds not here");
    expect(sourcesLabel(1)).toBe("laptop");
    expect(sourcesLabel(2)).toBe("phone + laptop");
    expect(sourcesLabel(3)).toBe("2 phones + laptop");
  });
});
