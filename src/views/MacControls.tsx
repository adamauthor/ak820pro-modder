import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { PageHeader } from "../components/Layout";
import { Button, Card, ErrorBanner } from "../components/ui";
import { formatError } from "../errors";
import { MacConfig, MacStatus, useMacControls } from "../mac-controls";

const INPUT = "rounded border border-line bg-surface-base px-3 py-2 text-sm text-fg-0";

export function MacControls() {
  const live = useMacControls();
  const [status, setStatus] = useState<MacStatus | null>(live);
  const [config, setConfig] = useState<MacConfig | null>(live?.config ?? null);
  const [dirty, setDirty] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState("");
  useEffect(() => { if (live) { setStatus(live); setConfig(previous => previous ?? live.config); } }, [live]);
  function change(value: Partial<MacConfig>) { if (config) { setConfig({ ...config, ...value }); setDirty(true); setNotice(""); } }
  async function run(command: string, args?: Record<string, unknown>) {
    setBusy(true); setError(null);
    try { const result = await invoke<MacStatus>(command, args); setStatus(result); return result; }
    catch (e) { setError(formatError(e)); }
    finally { setBusy(false); }
  }
  async function save() {
    const result = await run("mac_controls_save", { config });
    if (result) { setConfig(result.config); setDirty(false); setNotice("Saved on this Mac."); window.dispatchEvent(new Event("mac-controls-updated")); }
  }
  if (!status) return <p className="text-sm text-fg-2">Loading Mac controls…</p>;
  if (!status.supported) return <p className="text-sm text-fg-2">These controls require macOS.</p>;
  return <div className="space-y-5">
    <PageHeader title="Mac controls" description="Knob, YouTube Music and launch at login — all in AK820 Pro Modder."
      action={<Button disabled={busy} onClick={() => void run("mac_controls_refresh")}>Refresh</Button>} />
    <ErrorBanner>{error || status.error}</ErrorBanner>
    {notice && <p role="status" className="text-sm text-good">{notice}</p>}
    <Card title="Keyboard connection">
      {status.devices.length ? <ul className="space-y-2 text-sm">{status.devices.map(device => <li key={device.profile}>{device.product} · {device.transport}</li>)}</ul>
        : <p className="text-sm text-fg-2">Connect your AK820 by USB or Bluetooth. Saved knob settings apply automatically when it reconnects.</p>}
      <p className="mt-3 text-sm text-fg-2">Lighting and TFT uploads use USB. Mac key mappings, macros and the knob can also work over Bluetooth.</p>
    </Card>
    {config && <Card title="Knob click">
      <div className="space-y-4">
        <label className="flex flex-wrap items-center gap-3 text-sm">Action
          <select aria-label="Knob click action" className={INPUT} value={config.knob_mode} disabled={busy} onChange={e => change({ knob_mode: e.target.value as MacConfig["knob_mode"] })}>
            <option value="original">Restore previous action</option>
            <option value="play_pause">Play / pause</option>
            <option value="youtube_music" disabled={!status.youtube_supported}>Play / pause, or start YouTube Music</option>
          </select>
        </label>
        <p className="text-sm text-fg-2">Click to pause or resume YouTube Music, even after closing its window. An empty queue starts Liked Music. Already-playing Music or Spotify can be paused directly. Turning the knob still controls volume.</p>
        {config.knob_mode === "youtube_music" && <>
          <label className="flex flex-wrap items-center gap-3 text-sm">YouTube Music
            <select aria-label="YouTube Music application" className={INPUT} value={config.youtube_app_id ?? status.youtube_app_id ?? ""} disabled={busy} onChange={e => change({ youtube_app_id: e.target.value || null })}>
              <option value="">Find automatically</option>
              {status.youtube_apps.map(app => <option key={app.id} value={app.id}>{app.name}</option>)}
            </select>
          </label>
          {!status.youtube_apps.length && <p className="text-sm text-warn">Open music.youtube.com in Safari, choose File → Add to Dock, then Refresh here.</p>}
          <p className="text-sm text-fg-2">Accessibility: {status.accessibility ? "allowed" : "enable AK820 Pro Modder in System Settings → Privacy & Security → Accessibility, then use Stop controls and quit in the AK820 menu, and reopen the app"}. Requires macOS 14.2 or later and the Safari YouTube Music web app.</p>
        </>}
        <label className="flex items-center gap-3 text-sm"><input type="checkbox" className="accent-accent-500" disabled={busy} checked={config.launch_at_login} onChange={e => change({ launch_at_login: e.target.checked })} />Launch at login</label>
        <div className="flex flex-wrap gap-2">
          <Button variant="primary" disabled={busy || !dirty} onClick={() => void save()}>Save Mac controls</Button>
          <Button disabled={busy || dirty || status.config.knob_mode !== "youtube_music" || !status.accessibility} onClick={() => void run("mac_controls_action", { liked: false })}>Test knob action</Button>
          <Button disabled={busy || dirty || status.config.knob_mode !== "youtube_music" || !status.accessibility} onClick={() => void run("mac_controls_action", { liked: true })}>Play Liked Music</Button>
        </div>
        <p role="status" className="text-sm text-fg-1">{status.message}</p>
      </div>
    </Card>}
    <p className="text-sm text-fg-2">Close the window or press ⌘Q to keep controls and shortcuts running in the menu bar, without a Dock icon. Reopen settings from AK820. To stop everything, choose Stop controls and quit in that menu. These settings are saved on your Mac.</p>
  </div>;
}

export function UsbRequired() {
  return <Card title="Connect USB for keyboard settings">
    <p className="text-sm text-fg-2">Your keyboard is connected by Bluetooth. Lighting, system settings and TFT image/GIF uploads need the USB connection. Mac controls, key mappings and macros remain available.</p>
  </Card>;
}
