import { createContext, useContext } from "react";
import { invoke } from "@tauri-apps/api/core";
export type ClickAction = "toggle" | "next" | "previous" | "open" | "none";
export type TurnAction =
  | "volume"
  | "seek"
  | "zoom"
  | "tabs"
  | "scroll"
  | "none";
export interface GestureBindings {
  single: ClickAction;
  double: ClickAction;
  triple: ClickAction;
  rotation: TurnAction;
  shift_rotation: TurnAction;
}
export const DEFAULT_BINDINGS: GestureBindings = {
  single: "toggle",
  double: "next",
  triple: "previous",
  rotation: "volume",
  shift_rotation: "seek",
};
export interface AppProfile {
  id: string;
  name: string;
  bundle_id: string;
  enabled: boolean;
  bindings: GestureBindings;
  remaps: { source: number; destination: number }[];
}
export interface MacConfig {
  knob_mode: "original" | "play_pause" | "youtube_music" | "smart";
  launch_at_login: boolean;
  youtube_app_id: string | null;
  music_service: "youtube_music" | "spotify" | "apple_music";
  follow_playing: boolean;
  capture_rotation: boolean;
  click_interval_ms: number;
  bindings: GestureBindings;
  profiles: AppProfile[];
}
export interface MacStatus {
  supported: boolean;
  youtube_supported: boolean;
  accessibility: boolean;
  hotkey_active: boolean;
  devices: { profile: string; product: string; transport: string }[];
  youtube_apps: { id: string; name: string }[];
  youtube_app_id: string | null;
  installed_apps: { id: string; name: string }[];
  active_app: string | null;
  active_profile: string | null;
  preview: boolean;
  events: {
    id: number;
    gesture: string;
    action: string;
    profile: string;
    preview: boolean;
  }[];
  message: string;
  error: string | null;
  config: MacConfig;
}
export const MacControlsContext = createContext<MacStatus | null>(null);
export const useMacControls = () => useContext(MacControlsContext);

// No browser fixture or alternate backend can be selected in a release build.
export const controlsDemo =
  import.meta.env.DEV && import.meta.env.VITE_MAC_CONTROLS_DEMO === "1";
export async function invokeMac<T = MacStatus>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (controlsDemo)
    return (await import("./dev-mac-preview")).previewRequest(
      command,
      args,
    ) as Promise<T>;
  return invoke<T>(command, args);
}
