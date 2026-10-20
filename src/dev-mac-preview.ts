// Opt-in development fixture for reviewing the settings UI in a browser.
// Imported only with import.meta.env.DEV && VITE_MAC_CONTROLS_DEMO=1.
import { DEFAULT_BINDINGS, type MacStatus } from "./mac-controls";
const snapshot: MacStatus = {
  supported: true,
  youtube_supported: true,
  accessibility: true,
  hotkey_active: true,
  music_busy: false,
  music_automation: "not_needed",
  devices: [
    { profile: "bluetooth-1", product: "AK820 5.1-1", transport: "Bluetooth" },
  ],
  youtube_apps: [{ id: "com.apple.Safari.WebApp.preview", name: "YT Music" }],
  youtube_app_id: "com.apple.Safari.WebApp.preview",
  installed_apps: [
    { id: "com.apple.Music", name: "Music" },
    { id: "com.google.Chrome", name: "Google Chrome" },
    { id: "com.apple.Safari", name: "Safari" },
  ],
  active_app: "io.github.wsclx.ak820pro-modder",
  active_profile: null,
  preview: false,
  events: [],
  message: "Interface preview — no hardware commands",
  error: null,
  config: {
    knob_mode: "smart",
    launch_at_login: true,
    youtube_app_id: null,
    music_service: "youtube_music",
    follow_playing: true,
    capture_rotation: true,
    click_interval_ms: 300,
    bindings: { ...DEFAULT_BINDINGS },
    profiles: [],
  },
};
let previewUntil = 0;
export async function previewRequest(
  command: string,
  args?: Record<string, unknown>,
): Promise<MacStatus> {
  if (command === "mac_controls_save")
    snapshot.config = structuredClone(args?.config as MacStatus["config"]);
  if (command === "mac_controls_preview")
    previewUntil = args?.enabled ? performance.now() + 30000 : 0;
  snapshot.preview = performance.now() < previewUntil;
  return structuredClone(snapshot);
}
