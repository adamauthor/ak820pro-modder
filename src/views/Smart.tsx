import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { PageHeader } from "../components/Layout";
import { Badge, Button, Card, ErrorBanner, Toggle } from "../components/ui";
import { formatError } from "../errors";
import { t } from "../i18n";
import { useMacControls } from "../mac-controls";
import { SettingsTransfer } from "../components/SettingsTransfer";

const INPUT = "control-input";

interface Lighting {
  mode: string;
  color: string;
  secondary: string | null;
  color_mode: number;
  effect_mode_type: number;
  brightness: number;
  speed: number;
  direction: "left" | "down" | "up" | "right";
}
interface AppLighting { bundle_id: string; name: string; lighting: Lighting }
interface SmartConfig {
  teams_enabled: boolean;
  teams_lighting: Lighting;
  telegram_enabled: boolean;
  telegram_color: string;
  telegram_flashes: number;
  apps_enabled: boolean;
  app_rules: AppLighting[];
}
interface SmartStatus {
  config: SmartConfig;
  teams_active: boolean;
  usb: boolean;
  battery: number | null;
  battery_source: string;
}
type Phase = "idle" | "focus" | "short_break" | "long_break";
interface Snapshot { phase: Phase; paused: boolean; remaining_secs: number; total_secs: number; round: number; completed_today: number }
interface PomodoroConfig {
  focus_minutes: number;
  short_break_minutes: number;
  long_break_minutes: number;
  rounds_before_long_break: number;
  auto_start_breaks: boolean;
  auto_start_focus: boolean;
  color: string;
  notifications: boolean;
  anime_screen: boolean;
}
interface AnimeConfig {
  character: string;
  animated: boolean;
  city: string;
  consent: boolean;
  auto: boolean;
  refresh_minutes: number;
  rotation: string[];
  on_track_change: boolean;
}

const lighting = (color: string, mode = "static", brightness = 4): Lighting => ({
  mode, color, secondary: null, color_mode: 0, effect_mode_type: 0, brightness, speed: mode === "breath" ? 2 : 0, direction: "left",
});
const phaseName = (phase: Phase) => ({
  idle: t("Ready", "Готов"),
  focus: t("Focus", "Фокус"),
  short_break: t("Short break", "Короткий перерыв"),
  long_break: t("Long break", "Длинный перерыв"),
})[phase];
const sceneNames = () => ({
  daily: t("Greeting card", "Открытка дня"),
  weather: t("Weather", "Погода"),
  now_playing: t("Now playing", "Сейчас играет"),
});
const clock = (s: number) => `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;

function ColorInput({ value, onChange, label }: { value: string; onChange: (v: string) => void; label: string }) {
  return <input aria-label={label} type="color" className="h-8 w-12 cursor-pointer rounded border border-line bg-transparent"
    value={`#${value}`} onChange={e => onChange(e.target.value.slice(1).toUpperCase())} />;
}
function NumberInput({ value, onChange, min, max, label }: { value: number; onChange: (v: number) => void; min: number; max: number; label: string }) {
  return <label className="flex items-center justify-between gap-3 text-sm">{label}
    <input type="number" className={`${INPUT} w-24`} min={min} max={max} value={value}
      onChange={e => onChange(Math.max(min, Math.min(max, Number(e.target.value) || min)))} /></label>;
}

export function Smart() {
  const mac = useMacControls();
  const [status, setStatus] = useState<SmartStatus | null>(null);
  const [smart, setSmart] = useState<SmartConfig | null>(null);
  const [timer, setTimer] = useState<Snapshot | null>(null);
  const [pomodoro, setPomodoro] = useState<PomodoroConfig | null>(null);
  const [anime, setAnime] = useState<AnimeConfig | null>(null);
  const [characters, setCharacters] = useState<[string, string][]>([]);
  const [preview, setPreview] = useState<{ scene: string; frames: string[] } | null>(null);
  const [frame, setFrame] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState("");
  const [rule, setRule] = useState("");

  useEffect(() => {
    let alive = true;
    const poll = () => {
      void invoke<SmartStatus>("smart_status").then(s => { if (alive) { setStatus(s); setSmart(prev => prev ?? s.config); } }).catch(() => {});
      void invoke<[Snapshot, PomodoroConfig]>("pomodoro_status").then(([snap, cfg]) => { if (alive) { setTimer(snap); setPomodoro(prev => prev ?? cfg); } }).catch(() => {});
    };
    poll();
    void invoke<AnimeConfig>("anime_config").then(setAnime).catch(e => setError(formatError(e)));
    void invoke<[string, string][]>("anime_characters").then(setCharacters).catch(() => {});
    const id = window.setInterval(poll, 1000);
    return () => { alive = false; window.clearInterval(id); };
  }, []);
  useEffect(() => {
    if (!preview || preview.frames.length < 2) return;
    const id = window.setInterval(() => setFrame(f => (f + 1) % preview.frames.length), 250);
    return () => window.clearInterval(id);
  }, [preview]);

  async function run<T>(command: string, args?: Record<string, unknown>, done?: string): Promise<T | undefined> {
    setBusy(true); setError(null); setNotice("");
    try { const value = await invoke<T>(command, args); if (done) setNotice(done); return value; }
    catch (e) { setError(formatError(e)); }
    finally { setBusy(false); }
  }

  const usb = status?.usb === true;
  const apps = mac?.installed_apps ?? [];
  return <div className="space-y-5">
    <PageHeader title={t("Smart features", "Умные функции")}
      description={t("Lighting that reacts to Teams calls and Telegram, a red Pomodoro timer and anime cards for the screen.",
        "Подсветка реагирует на звонки Teams и Telegram, красный таймер помодоро и аниме-открытки на экран.")} />
    <ErrorBanner>{error}</ErrorBanner>
    {notice && <p role="status" className="text-sm text-good">{notice}</p>}
    <div className="flex flex-wrap gap-2">
      <Badge tone={usb ? "good" : "warn"}>{usb ? t("USB connected", "USB подключён") : t("Lighting and screen need USB", "Подсветке и экрану нужен USB")}</Badge>
      {status?.battery != null && <Badge tone={status.battery <= 20 ? "warn" : "neutral"}>{t("Battery", "Батарея")} {status.battery}%</Badge>}
      {status?.teams_active && <Badge tone="accent">{t("Teams call", "Звонок Teams")}</Badge>}
    </div>

    {smart && <Card title={t("Smart lighting", "Умная подсветка")} action={
      <Button variant="primary" disabled={busy} onClick={() => void run("smart_save", { config: smart }, t("Saved.", "Сохранено."))}>{t("Save", "Сохранить")}</Button>}>
      <div className="space-y-5">
        <div className="space-y-2">
          <Toggle checked={smart.teams_enabled} onChange={v => setSmart({ ...smart, teams_enabled: v })}>{t("Microsoft Teams call", "Звонок в Microsoft Teams")}</Toggle>
          <div className="flex flex-wrap items-center gap-3 pl-11 text-sm text-fg-2">
            <ColorInput label={t("Teams colour", "Цвет Teams")} value={smart.teams_lighting.color} onChange={c => setSmart({ ...smart, teams_lighting: { ...smart.teams_lighting, color: c } })} />
            <select aria-label={t("Teams effect", "Эффект Teams")} className={INPUT} value={smart.teams_lighting.mode}
              onChange={e => setSmart({ ...smart, teams_lighting: lighting(smart.teams_lighting.color, e.target.value, smart.teams_lighting.brightness) })}>
              <option value="breath">{t("Breathing", "Дыхание")}</option>
              <option value="static">{t("Steady", "Постоянно")}</option>
            </select>
            <span>{t("While Teams uses the microphone. Needs macOS 14.2+.", "Пока Teams использует микрофон. Нужна macOS 14.2+.")}</span>
          </div>
        </div>
        <div className="space-y-2">
          <Toggle checked={smart.telegram_enabled} onChange={v => setSmart({ ...smart, telegram_enabled: v })}>{t("Flash on Telegram messages", "Мигать при сообщениях Telegram")}</Toggle>
          <div className="flex flex-wrap items-center gap-3 pl-11 text-sm text-fg-2">
            <ColorInput label={t("Telegram colour", "Цвет Telegram")} value={smart.telegram_color} onChange={c => setSmart({ ...smart, telegram_color: c })} />
            <NumberInput label={t("Flashes", "Вспышек")} min={1} max={10} value={smart.telegram_flashes} onChange={v => setSmart({ ...smart, telegram_flashes: v })} />
            <span>{t("Reads the unread badge on Telegram's Dock icon (Accessibility).", "Следит за счётчиком непрочитанных на иконке Telegram в Dock (Универсальный доступ).")}</span>
          </div>
        </div>
        <div className="space-y-3">
          <Toggle checked={smart.apps_enabled} onChange={v => setSmart({ ...smart, apps_enabled: v })}>{t("Lighting per app", "Подсветка по приложениям")}</Toggle>
          <div className="space-y-2 pl-11">
            {smart.app_rules.map((r, i) => <div key={r.bundle_id} className="flex flex-wrap items-center gap-3 text-sm">
              <span className="w-40 truncate">{r.name}</span>
              <ColorInput label={r.name} value={r.lighting.color} onChange={c => setSmart({ ...smart, app_rules: smart.app_rules.map((x, n) => n === i ? { ...x, lighting: { ...x.lighting, color: c } } : x) })} />
              <select aria-label={`${r.name} ${t("effect", "эффект")}`} className={INPUT} value={r.lighting.mode}
                onChange={e => setSmart({ ...smart, app_rules: smart.app_rules.map((x, n) => n === i ? { ...x, lighting: lighting(x.lighting.color, e.target.value, x.lighting.brightness) } : x) })}>
                <option value="static">{t("Steady", "Постоянно")}</option>
                <option value="breath">{t("Breathing", "Дыхание")}</option>
              </select>
              <Button size="sm" onClick={() => setSmart({ ...smart, app_rules: smart.app_rules.filter((_, n) => n !== i) })}>{t("Remove", "Удалить")}</Button>
            </div>)}
            <div className="flex flex-wrap gap-2">
              <select aria-label={t("App", "Приложение")} className={INPUT} value={rule} onChange={e => setRule(e.target.value)}>
                <option value="">{t("Choose an app…", "Выберите приложение…")}</option>
                {apps.filter(a => !smart.app_rules.some(r => r.bundle_id === a.id)).map(a => <option key={a.id} value={a.id}>{a.name}</option>)}
              </select>
              <Button disabled={!rule} onClick={() => {
                const app = apps.find(a => a.id === rule);
                if (app) setSmart({ ...smart, app_rules: [...smart.app_rules, { bundle_id: app.id, name: app.name, lighting: lighting("00AAFF") }] });
                setRule("");
              }}>{t("Add", "Добавить")}</Button>
            </div>
          </div>
        </div>
        <p className="text-xs text-fg-3">{t("Priority: Teams call → Pomodoro → app → your lighting from the Lighting tab.",
          "Приоритет: звонок Teams → помодоро → приложение → ваша подсветка из вкладки «Подсветка».")}</p>
      </div>
    </Card>}

    {pomodoro && timer && <Card title={t("Pomodoro", "Помодоро")} action={
      <Button variant="primary" disabled={busy} onClick={() => void run("pomodoro_save", { config: pomodoro }, t("Saved.", "Сохранено."))}>{t("Save", "Сохранить")}</Button>}>
      <div className="grid gap-6 md:grid-cols-[1fr_1.2fr]">
        <div className="space-y-3">
          <p className="kicker">{phaseName(timer.phase)}{timer.paused ? ` · ${t("paused", "пауза")}` : ""}</p>
          <p className="font-mono text-5xl tabular" style={{ color: timer.phase === "focus" ? `#${pomodoro.color}` : undefined }}>
            {clock(timer.phase === "idle" ? pomodoro.focus_minutes * 60 : timer.remaining_secs)}</p>
          <p className="text-sm text-fg-2">{t("Today", "Сегодня")}: {timer.completed_today} 🍅 · {t("round", "раунд")} {timer.round + 1}/{pomodoro.rounds_before_long_break}</p>
          <div className="flex flex-wrap gap-2">
            {timer.phase === "idle" && <Button variant="primary" onClick={() => void run("pomodoro_command", { command: "start" })}>{t("Start", "Начать")}</Button>}
            {timer.phase !== "idle" && <Button onClick={() => void run("pomodoro_command", { command: timer.paused ? "resume" : "pause" })}>{timer.paused ? t("Resume", "Продолжить") : t("Pause", "Пауза")}</Button>}
            {timer.phase !== "idle" && <Button onClick={() => void run("pomodoro_command", { command: "skip" })}>{t("Skip", "Пропустить")}</Button>}
            {timer.phase !== "idle" && <Button variant="danger" onClick={() => void run("pomodoro_command", { command: "stop" })}>{t("Stop", "Стоп")}</Button>}
          </div>
          <p className="text-xs text-fg-3">{t("During focus the keyboard glows red and dims as time runs out, breathing in the last minute. You can also bind Pomodoro to a knob click in Mac controls and control it from the menu bar.",
            "Во время фокуса клавиатура светится красным и тускнеет по мере истечения времени, в последнюю минуту «дышит». Помодоро можно назначить на клик колёсика в «Управлении Mac» и управлять из строки меню.")}</p>
        </div>
        <div className="space-y-3">
          <NumberInput label={t("Focus, min", "Фокус, мин")} min={1} max={180} value={pomodoro.focus_minutes} onChange={v => setPomodoro({ ...pomodoro, focus_minutes: v })} />
          <NumberInput label={t("Short break, min", "Короткий перерыв, мин")} min={1} max={180} value={pomodoro.short_break_minutes} onChange={v => setPomodoro({ ...pomodoro, short_break_minutes: v })} />
          <NumberInput label={t("Long break, min", "Длинный перерыв, мин")} min={1} max={180} value={pomodoro.long_break_minutes} onChange={v => setPomodoro({ ...pomodoro, long_break_minutes: v })} />
          <NumberInput label={t("Rounds before a long break", "Раундов до длинного перерыва")} min={1} max={12} value={pomodoro.rounds_before_long_break} onChange={v => setPomodoro({ ...pomodoro, rounds_before_long_break: v })} />
          <label className="flex items-center justify-between gap-3 text-sm">{t("Focus colour", "Цвет фокуса")}<ColorInput label={t("Focus colour", "Цвет фокуса")} value={pomodoro.color} onChange={c => setPomodoro({ ...pomodoro, color: c })} /></label>
          <Toggle checked={pomodoro.auto_start_breaks} onChange={v => setPomodoro({ ...pomodoro, auto_start_breaks: v })}>{t("Start breaks automatically", "Начинать перерывы автоматически")}</Toggle>
          <Toggle checked={pomodoro.auto_start_focus} onChange={v => setPomodoro({ ...pomodoro, auto_start_focus: v })}>{t("Start focus automatically", "Начинать фокус автоматически")}</Toggle>
          <Toggle checked={pomodoro.notifications} onChange={v => setPomodoro({ ...pomodoro, notifications: v })}>{t("Notifications", "Уведомления")}</Toggle>
          <Toggle checked={pomodoro.anime_screen} onChange={v => setPomodoro({ ...pomodoro, anime_screen: v })}>{t("Anime card on the screen at each phase", "Аниме-открытка на экране при смене этапа")}</Toggle>
        </div>
      </div>
    </Card>}

    {anime && <Card title={t("Anime screen", "Аниме-экран")} action={
      <Button variant="primary" disabled={busy} onClick={() => void run("anime_save", { config: anime }, t("Saved.", "Сохранено."))}>{t("Save", "Сохранить")}</Button>}>
      <div className="grid gap-6 md:grid-cols-[auto_1fr]">
        <div className="space-y-2">
          <div className="grid h-[256px] w-[256px] place-items-center overflow-hidden rounded-lg border border-line bg-black">
            {preview ? <img alt={t("Preview", "Предпросмотр")} src={preview.frames[frame % preview.frames.length]} width={256} height={256} style={{ imageRendering: "pixelated" }} />
              : <span className="px-6 text-center text-xs text-fg-3">{t("Choose a card to preview it", "Выберите открытку для предпросмотра")}</span>}
          </div>
          {preview && <Button variant="primary" className="w-full" disabled={busy || !usb || !anime.consent}
            onClick={() => void run("anime_upload", { scene: preview.scene }, t("Uploaded to the keyboard.", "Загружено на клавиатуру."))}>{t("Send to keyboard", "Отправить на клавиатуру")}</Button>}
        </div>
        <div className="space-y-3">
          <div className="flex flex-wrap gap-2">
            {Object.entries(sceneNames()).map(([id, name]) => <Button key={id} disabled={busy}
              onClick={async () => { const frames = await run<string[]>("anime_preview", { scene: id }); if (frames) { setFrame(0); setPreview({ scene: id, frames }); } }}>{name}</Button>)}
          </div>
          <label className="flex items-center justify-between gap-3 text-sm">{t("Character", "Персонаж")}
            <select className={INPUT} value={anime.character} onChange={e => setAnime({ ...anime, character: e.target.value })}>
              {characters.map(([id, name]) => <option key={id} value={id}>{name}</option>)}
            </select></label>
          <label className="flex items-center justify-between gap-3 text-sm">{t("City for weather", "Город для погоды")}
            <input className={INPUT} value={anime.city} placeholder="Almaty" onChange={e => setAnime({ ...anime, city: e.target.value })} /></label>
          <Toggle checked={anime.animated} onChange={v => setAnime({ ...anime, animated: v })}>{t("Animated (blinking, petals, rain)", "Анимация (моргание, лепестки, дождь)")}</Toggle>
          <Toggle checked={anime.consent} onChange={v => setAnime({ ...anime, consent: v })}>{t("Allow uploads that replace the keyboard's stored animation", "Разрешить загрузки, заменяющие сохранённую анимацию клавиатуры")}</Toggle>
          <Toggle checked={anime.auto} disabled={!anime.consent} onChange={v => setAnime({ ...anime, auto: v })}>{t("Refresh automatically", "Обновлять автоматически")}</Toggle>
          <NumberInput label={t("Every, min (15+)", "Каждые, мин (от 15)")} min={15} max={1440} value={anime.refresh_minutes} onChange={v => setAnime({ ...anime, refresh_minutes: v })} />
          <div className="flex flex-wrap gap-4 text-sm">{Object.entries(sceneNames()).map(([id, name]) =>
            <label key={id} className="flex items-center gap-2"><input type="checkbox" checked={anime.rotation.includes(id)}
              onChange={e => setAnime({ ...anime, rotation: e.target.checked ? [...anime.rotation, id] : anime.rotation.filter(x => x !== id) })} />{name}</label>)}</div>
          <Toggle checked={anime.on_track_change} disabled={!anime.consent} onChange={v => setAnime({ ...anime, on_track_change: v })}>{t("New card when the track changes (at most every 5 min)", "Новая открытка при смене трека (не чаще раза в 5 минут)")}</Toggle>
          <p className="text-xs text-fg-3">{t("Each upload rewrites the keyboard's flash, so automatic refreshes are limited. Weather comes from Open-Meteo; only the city name is sent.",
            "Каждая загрузка перезаписывает флеш-память клавиатуры, поэтому автообновление ограничено. Погода — от Open-Meteo, передаётся только название города.")}</p>
        </div>
      </div>
    </Card>}
    <SettingsTransfer />
  </div>;
}
