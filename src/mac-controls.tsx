import { createContext, useContext } from "react";

export type KnobMode = "original" | "play_pause" | "youtube_music";
export interface MacConfig {
  knob_mode: KnobMode;
  launch_at_login: boolean;
  youtube_app_id: string | null;
}
export interface MacStatus {
  supported: boolean;
  youtube_supported: boolean;
  accessibility: boolean;
  hotkey_active: boolean;
  devices: { profile: string; product: string; transport: string }[];
  youtube_apps: { id: string; name: string }[];
  youtube_app_id: string | null;
  message: string;
  error: string | null;
  config: MacConfig;
}
export const MacControlsContext = createContext<MacStatus | null>(null);
export const useMacControls = () => useContext(MacControlsContext);
