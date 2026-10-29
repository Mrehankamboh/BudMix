// Mirrors the serde types in crates/budslink-core and src-tauri.

export type ConnectionState = "disconnected" | "connecting" | "connected" | "disconnecting";

export type AudioProfile = "stereo" | "handsFree" | "leAudio" | "unknown";

export interface DeviceView {
  id: string;
  name: string;
  profiles: AudioProfile[];
  state: ConnectionState;
  reconnectAttempt: number | null;
  lastError: string | null;
  /** The device connected most recently; it may still be connected. */
  lastConnected: boolean;
}

export type BridgeState = "off" | "connecting" | "streaming" | "waiting" | "paused";

/** A paired phone whose audio can play through the laptop (bridge mode). */
export interface SourceView {
  id: string;
  name: string;
  /** The user wants this phone's audio in the buds. */
  enabled: boolean;
  state: BridgeState;
  lastError: string | null;
}

export type UnavailableReason = "noAudioService" | "lowEnergyOnly";

/** A paired phone that can't stream to this laptop, listed with the reason. */
export interface UnavailablePhone {
  name: string;
  reason: UnavailableReason;
}

export interface ReconnectPolicy {
  initialDelayMs: number;
  maxDelayMs: number;
  maxAttempts: number;
}

export interface Settings {
  version: number;
  autoReconnect: boolean;
  reconnect: ReconnectPolicy;
  connectTimeoutMs: number;
  preferredDevice: string | null;
  toggleHotkey: string | null;
  startWithWindows: boolean;
  bridgedPhones: string[];
  /** Make the connected buds the default output so all laptop audio plays in them. */
  routeLaptopAudio: boolean;
  /** The output to give back later; owned by the app, not the UI. */
  savedOutput: string | null;
  /** The first-run setup check was finished or skipped once. */
  setupDone: boolean;
  /** Where phone audio comes from. */
  phoneMode: PhoneMode;
  /** Phones allowed to cast without asking; owned by the app. */
  allowedPhones: string[];
  volumes: Volumes;
  /** Show the phone's screen in a small window while it casts. */
  showCastVideo: boolean;
  phonesSeeded: boolean;
}

/** Auto: Wi-Fi while the phone casts, Bluetooth otherwise. */
export type PhoneMode = "auto" | "wifiOnly" | "bluetoothOnly";

export interface Level {
  /** 0 to 100. */
  percent: number;
  muted: boolean;
}

export type VolumeSource = "master" | "laptop" | "phoneWifi" | "phoneBluetooth";

export interface Volumes {
  master: Level;
  laptop: Level;
  phoneWifi: Level;
  phoneBluetooth: Level;
}

export type ReceiverState = "off" | "notPrivate" | "starting" | "ready" | "unsupported" | "failed";

export type CastPhase = "idle" | "needsApproval" | "connecting" | "playing";

/** Phone audio over Wi-Fi (Miracast cast) and the Bluetooth rule. */
export interface CastStatus {
  receiver: ReceiverState;
  message: string | null;
  phone: string | null;
  phase: CastPhase;
  pin: string | null;
  allowed: string[];
  mode: PhoneMode;
  /** The Bluetooth phone link is held off (the phone casts, or Wi-Fi only). */
  bluetoothSuspended: boolean;
  videoVisible: boolean;
  lastError: string | null;
}

export type CheckId =
  | "bluetooth"
  | "budsPaired"
  | "phonePaired"
  | "networkPrivate"
  | "firewall"
  | "miracast"
  | "wirelessDisplay";

export type CheckStatus = "ok" | "warning" | "problem" | "optional" | "unknown";

/** What a check's "Fix" button does. Admin fixes show one Windows prompt. */
export type Fix =
  | { kind: "openSettings"; uri: string }
  | { kind: "makeNetworkPrivate"; network: string }
  | { kind: "addFirewallRule" }
  | { kind: "installWirelessDisplay" };

/** One prerequisite of the setup check. */
export interface CheckResult {
  id: CheckId;
  status: CheckStatus;
  title: string;
  detail: string;
  fix: Fix | null;
}

/** A playback device as Windows names it. */
export interface Endpoint {
  id: string;
  name: string;
}

/** Laptop audio: where it plays and whether the buds are in call mode. */
export interface AudioView {
  routing: boolean;
  buds: string | null;
  budsOutput: Endpoint | null;
  defaultOutput: Endpoint | null;
  /** The buds are the default output, so laptop audio plays in them. */
  laptopInBuds: boolean;
  /** An app uses the buds' mic: Windows switched them to low-quality call mode (HFP). */
  callMode: boolean;
  savedOutput: string | null;
  lastError: string | null;
}

export interface AppInfo {
  /** "windows" for real devices, "demo" for the simulated backend. */
  backend: string;
  version: string;
  /** Window uses the Windows 11 Mica backdrop, so the page must be transparent. */
  mica: boolean;
  /** Problems applying settings, e.g. a hotkey already taken by another app. */
  warnings: string[];
}
