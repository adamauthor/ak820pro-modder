import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Automations } from "./views/Automations";
import { Connect } from "./views/Connect";
import { Keymap } from "./views/Keymap";
import { Lighting } from "./views/Lighting";
import { Macros } from "./views/Macros";
import { Presets } from "./views/Presets";
import { System } from "./views/System";
import { Tft } from "./views/Tft";
import { CapabilityContext, DeviceCapabilities, UnavailableFeature } from "./capabilities";
import { HostKeymap, HostMacros } from "./views/HostKeyboard";
import { LegacySystem } from "./views/LegacySystem";
import { MacControls, UsbRequired } from "./views/MacControls";
import { MacControlsContext, MacStatus } from "./mac-controls";
import { Music } from "lucide-react";
import { ICLOUD_SYNC_ENABLED_KEY } from "./components/SyncCard";
import {
  Layout,
  Plug,
  Bulb,
  Settings,
  Keyboard,
  Macro,
  Screen,
  Automation,
  Preset,
  type NavItem,
} from "./components/Layout";

type Tab =
  | "connect"
  | "lighting"
  | "system"
  | "maccontrols"
  | "keymap"
  | "macros"
  | "automations"
  | "presets"
  | "tft";

const ICON_PROPS = { size: 16, strokeWidth: 1.6 } as const;

const NAV: NavItem<Tab>[] = [
  { id: "connect", label: "Connectivity", icon: <Plug {...ICON_PROPS} /> },
  { id: "lighting", label: "Lighting", icon: <Bulb {...ICON_PROPS} /> },
  { id: "system", label: "System", icon: <Settings {...ICON_PROPS} /> },
  { id: "maccontrols", label: "Mac controls", icon: <Music {...ICON_PROPS} /> },
  { id: "keymap", label: "Keymap", icon: <Keyboard {...ICON_PROPS} /> },
  { id: "macros", label: "Macros", icon: <Macro {...ICON_PROPS} /> },
  { id: "automations", label: "Automations", icon: <Automation {...ICON_PROPS} /> },
  { id: "presets", label: "Presets", icon: <Preset {...ICON_PROPS} /> },
  { id: "tft", label: "TFT Display", icon: <Screen {...ICON_PROPS} /> },
];

interface ProbeReport {
  connected: boolean;
  interface: number;
  product: string | null;
}

export default function App() {
  const [tab, setTab] = useState<Tab>("connect");
  const [capabilities, setCapabilities] = useState<DeviceCapabilities | null>(null);
  const [probe, setProbe] = useState<ProbeReport | null>(null);
  const [mac, setMac] = useState<MacStatus | null>(null);
  useEffect(() => {
    let alive = true;
    const refresh = () => { void invoke<MacStatus>("mac_controls_status").then(value => { if (alive) setMac(value); }).catch(() => {}); };
    refresh();
    const timer = window.setInterval(refresh, 4000);
    window.addEventListener("mac-controls-updated", refresh);
    return () => { alive = false; window.clearInterval(timer); window.removeEventListener("mac-controls-updated", refresh); };
  }, []);

  // iCloud auto-pull on app start. Fire-and-forget — if the iCloud
  // copy of automations.json is newer than the local one, the backend
  // overwrites the local file before any view requests its contents.
  // The frontend never sees the in-flight state because all
  // automation-consuming views fetch on mount, and this effect runs
  // before any view-specific effect.
  useEffect(() => {
    let enabled = false;
    try {
      enabled = window.localStorage.getItem(ICLOUD_SYNC_ENABLED_KEY) === "true";
    } catch {
      /* private-browsing — ignore */
    }
    if (!enabled) return;
    void invoke("icloud_sync_pull").catch(() => {
      // Pull failure on startup shouldn't bubble — user sees the
      // error inline if they open the System → Sync card.
    });
  }, []);

  useEffect(() => {
    let alive = true;

    // Defer first probe by a frame so React finishes mounting cleanly before
    // the first IPC call. Without this we have seen WKWebView freezes that
    // we can't reproduce in Chromium — likely a startup-time ordering issue.
    const start = window.setTimeout(async function tick() {
      if (!alive) return;
      try {
        const p = await invoke<ProbeReport>("probe_device");
        const caps = p.connected ? await invoke<DeviceCapabilities>("get_capabilities") : null;
        if (alive) { setProbe(p); setCapabilities(caps); }
      } catch {
        if (alive) setProbe(null);
      }
      if (alive) window.setTimeout(tick, 4000);
    }, 200);

    return () => {
      alive = false;
      window.clearTimeout(start);
    };
  }, []);

  const reconnect = async () => {
    try { await invoke("force_reconnect"); } catch { /* ignored */ }
    try { setMac(await invoke<MacStatus>("mac_controls_refresh")); } catch { /* native status explains errors */ }
  };
  const wirelessOnly = !probe?.connected && mac?.devices.some(device => device.transport === "Bluetooth");
  const hostKeyboard = mac?.supported && (!probe?.connected || capabilities?.protocol === "legacy_feature");
  const connection = probe?.connected ? { connected: true, product: probe.product }
    : mac?.devices.length ? { connected: true, product: `${mac.devices[0].product} · ${mac.devices[0].transport}` }
    : probe ? { connected: false, product: probe.product } : undefined;

  return (
    <MacControlsContext.Provider value={mac}><CapabilityContext.Provider value={capabilities}><Layout
      nav={NAV}
      active={tab}
      onSelect={setTab}
      connection={connection}
      onReconnect={reconnect}
      wide={tab === "keymap"}
    >
      {tab === "connect" && <Connect onReconnect={reconnect} />}
      {tab === "lighting" && (wirelessOnly ? <UsbRequired /> : <Lighting />)}
      {tab === "system" && (wirelessOnly ? <UsbRequired /> : capabilities?.protocol === "legacy_feature" ? <LegacySystem /> : <System />)}
      {tab === "maccontrols" && <MacControls />}
      {tab === "keymap" && (hostKeyboard ? <HostKeymap /> : capabilities && !capabilities.keymap ? <UnavailableFeature name="Onboard key remapping" /> : <Keymap />)}
      {tab === "macros" && (hostKeyboard ? <HostMacros /> : capabilities && !capabilities.macros ? <UnavailableFeature name="Onboard macros" /> : <Macros />)}
      {tab === "automations" && <Automations />}
      {tab === "presets" && (wirelessOnly ? <UsbRequired /> : <Presets />)}
      {tab === "tft" && (wirelessOnly ? <UsbRequired /> : <Tft />)}
    </Layout></CapabilityContext.Provider></MacControlsContext.Provider>
  );
}
