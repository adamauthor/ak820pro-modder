import { useEffect, useState } from "react";
import {
  Check,
  ChevronRight,
  CircleHelp,
  Headphones,
  Keyboard,
  Layers,
  MousePointer2,
  Music2,
  Plus,
  Radio,
  RefreshCw,
  Settings2,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import { PageHeader } from "../components/Layout";
import { Badge, Button, Card, ErrorBanner } from "../components/ui";
import { formatError } from "../errors";
import {
  AppProfile,
  GestureBindings,
  MacConfig,
  MacStatus,
  useMacControls,
  invokeMac,
  controlsDemo,
} from "../mac-controls";
import { t, useI18n } from "../i18n";
import { hidName } from "../data/hid-usage-names";

const INPUT = "control-input";
const KEYS = [
  ...Array.from({ length: 112 }, (_, i) => i + 4),
  ...Array.from({ length: 8 }, (_, i) => i + 224),
].filter((u) => ![109, 110, 111].includes(u));
const clickActions = () => [
  ["toggle", t("Play / pause", "Воспроизведение / пауза")],
  ["next", t("Next track", "Следующий трек")],
  ["previous", t("Previous track", "Предыдущий трек")],
  ["open", t("Open music app", "Открыть плеер")],
  ["none", t("No action", "Без действия")],
];
const turnActions = () => [
  ["volume", t("Volume", "Громкость")],
  ["seek", t("Seek ±5 seconds", "Перемотка ±5 секунд")],
  ["zoom", t("Zoom / font size", "Масштаб / размер шрифта")],
  ["tabs", t("Switch tabs", "Переключение вкладок")],
  ["scroll", t("Scroll", "Прокрутка")],
  ["none", t("No action", "Без действия")],
];
const gestures = () =>
  [
    ["single", t("Single click", "Один клик"), "1×"],
    ["double", t("Double click", "Двойной клик"), "2×"],
    ["triple", t("Triple click", "Тройной клик"), "3×"],
    ["rotation", t("Turn", "Поворот"), "↻"],
    ["shift_rotation", t("Shift + turn", "Shift + поворот"), "⇧ ↻"],
  ] as const;
function actionName(value: string) {
  return (
    [...clickActions(), ...turnActions()].find(
      (row) => row[0] === value,
    )?.[1] ?? value
  );
}
function SwitchRow({
  title,
  detail,
  checked,
  onChange,
  disabled,
}: {
  title: string;
  detail: string;
  checked: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <label className="setting-row cursor-pointer">
      <span className="pr-5">
        <span className="block font-medium text-fg-0">{title}</span>
        <span className="mt-1 block text-sm text-fg-2">{detail}</span>
      </span>
      <input
        type="checkbox"
        role="switch"
        className="switch-control"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        disabled={disabled}
      />
    </label>
  );
}
function BindingRows({
  value,
  onChange,
  rotation,
}: {
  value: GestureBindings;
  onChange: (value: GestureBindings) => void;
  rotation: boolean;
}) {
  return (
    <div className="divide-y divide-line-subtle">
      {gestures().map(([key, label, mark], index) => (
        <label
          key={key}
          className={`setting-row ${index > 2 && !rotation ? "opacity-50" : ""}`}
        >
          <span className="flex items-center gap-3">
            <span className="gesture-mark">{mark}</span>
            <span className="text-sm font-medium">{label}</span>
          </span>
          <select
            aria-label={label}
            className={INPUT}
            disabled={index > 2 && !rotation}
            value={value[key]}
            onChange={(e) => onChange({ ...value, [key]: e.target.value })}
          >
            {(index > 2 ? turnActions() : clickActions()).map(([id, name]) => (
              <option key={id} value={id}>
                {name}
              </option>
            ))}
          </select>
        </label>
      ))}
    </div>
  );
}

export function MacControls({ visible = true }: { visible?: boolean }) {
  const live = useMacControls();
  const { language, setLanguage } = useI18n();
  const [status, setStatus] = useState<MacStatus | null>(live);
  const [config, setConfig] = useState<MacConfig | null>(live?.config ?? null);
  const [section, setSection] = useState("wheel");
  const [selected, setSelected] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [appChoice, setAppChoice] = useState("");
  const [search, setSearch] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const dirty =
    config !== null &&
    status !== null &&
    JSON.stringify(config) !== JSON.stringify(status.config);
  useEffect(() => {
    if (live) {
      setStatus(live);
      setConfig((previous) => previous ?? live.config);
    }
  }, [live]);
  useEffect(() => {
    if (!visible) return;
    let alive = true;
    const timer = window.setInterval(() => {
      void invokeMac<MacStatus>("mac_controls_status")
        .then((value) => {
          if (alive) setStatus(value);
        })
        .catch(() => {});
    }, 1000);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [visible]);
  useEffect(() => {
    if (!visible && status?.preview)
      void invokeMac("mac_controls_preview", { enabled: false }).catch(
        () => {},
      );
  }, [visible, status?.preview]);
  function change(value: Partial<MacConfig>) {
    if (config) {
      setConfig({ ...config, ...value });
      setSaved(false);
    }
  }
  async function run(command: string, args?: Record<string, unknown>) {
    setBusy(true);
    setError(null);
    try {
      const result = await invokeMac<MacStatus>(command, args);
      setStatus(result);
      return result;
    } catch (e) {
      setError(formatError(e));
    } finally {
      setBusy(false);
    }
  }
  async function save() {
    const result = await run("mac_controls_save", { config });
    if (result) {
      setConfig(result.config);
      setSaved(true);
      window.dispatchEvent(new Event("mac-controls-updated"));
    }
  }
  function profileChange(value: Partial<AppProfile>) {
    if (config)
      change({
        profiles: config.profiles.map((p) =>
          p.id === selected ? { ...p, ...value } : p,
        ),
      });
  }
  function addProfile() {
    if (!config || !appChoice) return;
    const app = status?.installed_apps.find((a) => a.id === appChoice);
    if (!app) return;
    const profile: AppProfile = {
      id: crypto.randomUUID(),
      name: app.name,
      bundle_id: app.id,
      enabled: true,
      bindings: {
        ...config.bindings,
        rotation: /safari|chrome|firefox|code|cursor|zed/i.test(app.id)
          ? "zoom"
          : "volume",
      },
      remaps: [],
    };
    change({ profiles: [...config.profiles, profile] });
    setSelected(profile.id);
    setAdding(false);
    setAppChoice("");
    setSearch("");
  }
  if (!status || !config)
    return (
      <div className="control-loading">
        <Radio className="animate-pulse" />
        {t("Loading your controls…", "Загружаем ваши настройки…")}
      </div>
    );
  if (!status.supported)
    return (
      <p>
        {t("These controls require macOS.", "Эти настройки доступны на macOS.")}
      </p>
    );
  const advanced = ["youtube_music", "smart"].includes(config.knob_mode);
  const profile = config.profiles.find((p) => p.id === selected);
  const activeProfile = config.profiles.find(
    (p) => p.id === status.active_profile,
  );
  const availableApps = status.installed_apps.filter(
    (a) =>
      !config.profiles.some((p) => p.bundle_id === a.id) &&
      a.id !== "io.github.wsclx.ak820pro-modder" &&
      a.name.toLowerCase().includes(search.toLowerCase()),
  );
  const serviceOptions = [
    {
      id: "youtube_music" as const,
      name: "YouTube Music",
      symbol: "▶",
      color: "#e85454",
      installed: status.youtube_apps.length > 0,
      detail: t(
        "Safari web app · Liked Music fallback",
        "Веб-приложение Safari · «Мне понравилось»",
      ),
    },
    {
      id: "spotify" as const,
      name: "Spotify",
      symbol: "≋",
      color: "#1dbb63",
      installed: status.installed_apps.some(
        (a) => a.id === "com.spotify.client",
      ),
      detail: t(
        "Desktop app · resume your queue",
        "Приложение для Mac · текущая очередь",
      ),
    },
    {
      id: "apple_music" as const,
      name: "Apple Music",
      symbol: "♫",
      color: "#ec6287",
      installed: status.installed_apps.some((a) => a.id === "com.apple.Music"),
      detail: t(
        "Music for Mac · library and queue",
        "Музыка на Mac · медиатека и очередь",
      ),
    },
  ];
  return (
    <div className="control-page">
      {controlsDemo && (
        <div className="help-note">
          Interface preview — no hardware commands
        </div>
      )}
      <PageHeader
        kicker={t("MADE FOR YOUR AK820", "ВАША AK820 НА ВАШИХ УСЛОВИЯХ")}
        title={t("Make it yours.", "Настройте под себя.")}
        description={t(
          "One knob. Your music, your apps, your rhythm.",
          "Музыка, приложения и привычные действия — под рукой.",
        )}
        action={
          <Button
            disabled={busy}
            onClick={() => void run("mac_controls_refresh")}
          >
            <RefreshCw size={14} />
            {t("Refresh")}
          </Button>
        }
      />
      <div className="connection-strip">
        <span
          className={`h-2 w-2 rounded-full ${status.devices.length ? "bg-good" : "bg-fg-4"}`}
        />
        <span>
          {status.devices[0]
            ? `${status.devices[0].product} · ${status.devices[0].transport}`
            : t("Waiting for your keyboard", "Ожидаем клавиатуру")}
        </span>
        <span className="ml-auto flex items-center gap-2 text-fg-2">
          <Layers size={13} />
          {activeProfile?.name ?? t("Default profile", "Основной профиль")}
        </span>
      </div>
      <ErrorBanner>{error || status.error}</ErrorBanner>
      {!status.accessibility && advanced && (
        <div className="permission-callout">
          <ShieldCheck size={22} />
          <div className="flex-1">
            <strong>
              {t(
                "Allow control of music and shortcuts",
                "Разрешите управление музыкой и сочетаниями",
              )}
            </strong>
            <p className="mt-1 text-sm text-fg-2">
              {t(
                "Enable AK820 Pro Modder in Accessibility. If you just updated it, stop controls from the AK820 menu and reopen the app.",
                "Включите AK820 Pro Modder в «Универсальный доступ». После обновления полностью остановите приложение через меню AK820 и откройте снова.",
              )}
            </p>
          </div>
          <Button
            onClick={() => {
              void invokeMac("mac_controls_permission").catch((e) =>
                setError(formatError(e)),
              );
            }}
          >
            {t("Open settings", "Открыть настройки")}
          </Button>
        </div>
      )}
      <div
        role="tablist"
        onKeyDown={(e) => {
          const tabs = ["wheel", "music", "profiles", "general"];
          const index = tabs.indexOf(section);
          const next =
            e.key === "ArrowRight"
              ? (index + 1) % 4
              : e.key === "ArrowLeft"
                ? (index + 3) % 4
                : e.key === "Home"
                  ? 0
                  : e.key === "End"
                    ? 3
                    : -1;
          if (next >= 0) {
            e.preventDefault();
            setSection(tabs[next]);
            document.getElementById(`control-tab-${tabs[next]}`)?.focus();
          }
        }}
        aria-label={t("Control settings", "Настройки управления")}
        className="control-tabs"
      >
        {[
          ["wheel", t("Knob", "Колёсико"), <MousePointer2 size={15} />],
          ["music", t("Music", "Музыка"), <Headphones size={15} />],
          [
            "profiles",
            t("App profiles", "Профили приложений"),
            <Layers size={15} />,
          ],
          ["general", t("Preferences", "Параметры"), <Settings2 size={15} />],
        ].map(([id, name, icon]) => (
          <button
            key={id as string}
            role="tab"
            id={`control-tab-${id}`}
            aria-controls="control-panel"
            tabIndex={section === id ? 0 : -1}
            aria-selected={section === id}
            onClick={() => setSection(id as string)}
            className={section === id ? "active" : ""}
          >
            {icon}
            {name}
          </button>
        ))}
      </div>
      <div
        id="control-panel"
        role="tabpanel"
        aria-labelledby={`control-tab-${section}`}
        className="control-panel"
      >
        {section === "wheel" && (
          <>
            <div className="knob-hero">
              <div className="knob-art" aria-hidden="true">
                <div className="knob-rim">
                  <div className="knob-face">
                    <span />
                  </div>
                </div>
                <span className="knob-hint">AK820</span>
              </div>
              <div>
                <Badge tone={advanced ? "good" : "neutral"}>
                  {advanced
                    ? t("Custom controls", "Ваши действия")
                    : t("Standard controls", "Стандартное управление")}
                </Badge>
                <h2 className="mt-3 text-2xl font-semibold">
                  {t("More in every turn.", "Больше, чем громкость.")}
                </h2>
                <p className="mt-2 max-w-sm text-sm text-fg-2">
                  {t(
                    "Click for music. Turn for volume. Give every app its own controls.",
                    "Управляйте музыкой, меняйте громкость и назначайте отдельные действия для приложений.",
                  )}
                </p>
              </div>
            </div>
            <Card title={t("Knob behavior", "Режим колёсика")}>
              <label className="setting-row !pt-0">
                <span className="text-sm">
                  {t("Control mode", "Режим управления")}
                </span>
                <select
                  aria-label={t("Control mode", "Режим управления")}
                  className={INPUT}
                  value={
                    config.knob_mode === "youtube_music"
                      ? "smart"
                      : config.knob_mode
                  }
                  onChange={(e) =>
                    change({
                      knob_mode: e.target.value as MacConfig["knob_mode"],
                    })
                  }
                >
                  <option value="smart">
                    {t("Custom gestures & music", "Жесты и музыка")}
                  </option>
                  <option value="play_pause">
                    {t(
                      "System play / pause",
                      "Системное воспроизведение / пауза",
                    )}
                  </option>
                  <option value="original">
                    {t(
                      "Restore original behavior",
                      "Вернуть прежнее поведение",
                    )}
                  </option>
                </select>
              </label>
              {advanced && (
                <>
                  <BindingRows
                    value={config.bindings}
                    onChange={(bindings) => change({ bindings })}
                    rotation={config.capture_rotation}
                  />
                  <SwitchRow
                    checked={config.capture_rotation}
                    onChange={(capture_rotation) =>
                      change({ capture_rotation })
                    }
                    title={t("Customize turning", "Настраивать поворот")}
                    detail={t(
                      "Enables Shift + turn and per-app turning. F18–F20 are reserved while enabled.",
                      "Включает Shift + поворот и действия для приложений. При этом F18–F20 резервируются для колёсика.",
                    )}
                  />
                  <label className="setting-row !pb-0">
                    <span>
                      <span className="block text-sm font-medium">
                        {t("Multi-click timing", "Интервал между кликами")}
                      </span>
                      <span className="mt-1 block text-xs text-fg-2">
                        {t(
                          "Single clicks wait this long for a second click.",
                          "Одиночный клик ждёт второй в течение этого времени.",
                        )}
                      </span>
                    </span>
                    <span className="flex items-center gap-3">
                      <input
                        aria-label={t(
                          "Multi-click timing",
                          "Интервал между кликами",
                        )}
                        type="range"
                        min="220"
                        max="500"
                        step="20"
                        value={config.click_interval_ms}
                        onChange={(e) =>
                          change({ click_interval_ms: Number(e.target.value) })
                        }
                      />
                      <output className="w-16 text-right text-sm tabular">
                        {config.click_interval_ms} {t("ms", "мс")}
                      </output>
                    </span>
                  </label>
                </>
              )}
            </Card>
            {advanced && (
              <Card
                title={t("Try your gestures", "Проверьте жесты")}
                action={
                  <Button
                    disabled={busy || dirty || !status.hotkey_active}
                    onClick={() =>
                      void run("mac_controls_preview", {
                        enabled: !status.preview,
                      })
                    }
                  >
                    {status.preview
                      ? t("Finish test", "Завершить")
                      : t("Test for 30 seconds", "Проверить 30 секунд")}
                  </Button>
                }
              >
                <p className="text-sm text-fg-2">
                  {status.preview
                    ? t(
                        "Test mode is active: use the physical knob. Gestures are shown here without running their actions.",
                        "Проверка включена: нажимайте и вращайте колёсико. Жесты появятся здесь без выполнения действий.",
                      )
                    : t(
                        "Save changes first, then test without changing tracks or volume. Turning is detected when Customize turning is enabled.",
                        "Сначала сохраните настройки. В режиме проверки треки и громкость не меняются. Для проверки вращения включите «Настраивать поворот».",
                      )}
                </p>
                <div className="gesture-log" aria-live="polite">
                  {status.events.length ? (
                    status.events.slice(0, 4).map((e) => (
                      <div key={e.id} className="flex items-center gap-3 py-2">
                        <span className="h-1.5 w-1.5 rounded-full bg-accent-400" />
                        <span className="w-28 text-sm">
                          {gestures().find((g) => g[0] === e.gesture)?.[1] ??
                            ({
                              right: "↻",
                              left: "↺",
                              shift_right: "⇧ ↻",
                              shift_left: "⇧ ↺",
                            }[e.gesture] ||
                              e.gesture)}
                        </span>
                        <ChevronRight size={12} className="text-fg-3" />
                        <span className="flex-1 text-sm">
                          {actionName(e.action)}
                        </span>
                        <span className="text-xs text-fg-3">
                          {e.profile === "Default"
                            ? t("Default", "Основной")
                            : e.profile}
                        </span>
                      </div>
                    ))
                  ) : (
                    <span className="text-sm text-fg-3">
                      {t(
                        "Your next gesture will appear here.",
                        "Следующий жест появится здесь.",
                      )}
                    </span>
                  )}
                </div>
              </Card>
            )}
          </>
        )}
        {section === "music" && (
          <>
            <div className="section-intro">
              <Music2 size={22} />
              <div>
                <h2>
                  {t("Your music, your choice.", "Ваша музыка. Ваш выбор.")}
                </h2>
                <p>
                  {t(
                    "If nothing is playing, the knob opens your preferred app and resumes its queue.",
                    "Когда музыка не играет, колёсико открывает выбранный плеер и продолжает его очередь.",
                  )}
                </p>
              </div>
            </div>
            <div className="service-grid">
              {serviceOptions.map((service) => (
                <button
                  key={service.id}
                  className={`service-card ${config.music_service === service.id ? "selected" : ""}`}
                  onClick={() => change({ music_service: service.id })}
                  aria-pressed={config.music_service === service.id}
                >
                  <span
                    className="service-symbol"
                    style={{ background: service.color }}
                  >
                    {service.id === "spotify" ? (
                      <svg
                        width="28"
                        height="28"
                        viewBox="0 0 28 28"
                        fill="none"
                        aria-hidden="true"
                      >
                        <path
                          d="M6 10c5-2 11-1 16 1M7 14c4-1.5 9-1 13 1M8 18c3-1 7-.8 10 1"
                          stroke="white"
                          strokeWidth="2"
                          strokeLinecap="round"
                        />
                      </svg>
                    ) : (
                      service.symbol
                    )}
                  </span>
                  <span className="mt-4 block font-semibold">
                    {service.name}
                  </span>
                  <span className="mt-1 block min-h-10 text-xs text-fg-2">
                    {service.detail}
                  </span>
                  <span className="mt-4 flex items-center justify-between text-xs">
                    <span
                      className={service.installed ? "text-good" : "text-fg-3"}
                    >
                      {service.installed
                        ? t("Installed", "Установлено")
                        : t("Not installed", "Не установлено")}
                    </span>
                    {config.music_service === service.id && (
                      <Check size={15} className="text-accent-400" />
                    )}
                  </span>
                </button>
              ))}
            </div>
            {config.music_service === "youtube_music" && (
              <Card title="YouTube Music">
                <label className="setting-row !py-0">
                  <span className="text-sm">
                    {t("Safari web app", "Веб-приложение Safari")}
                  </span>
                  <select
                    className={INPUT}
                    aria-label="YouTube Music"
                    value={config.youtube_app_id ?? ""}
                    onChange={(e) =>
                      change({ youtube_app_id: e.target.value || null })
                    }
                  >
                    <option value="">
                      {t("Find automatically", "Найти автоматически")}
                    </option>
                    {status.youtube_apps.map((a) => (
                      <option key={a.id} value={a.id}>
                        {a.name}
                      </option>
                    ))}
                  </select>
                </label>
                <p className="mt-4 text-sm text-fg-2">
                  {status.youtube_apps.length
                    ? t(
                        "An empty queue starts Liked Music. Closing the player window is supported.",
                        "Если очередь пуста, запустится «Мне понравилось». Можно закрывать окно плеера крестиком.",
                      )
                    : t(
                        "Open music.youtube.com in Safari → File → Add to Dock. Then refresh this page.",
                        "Откройте music.youtube.com в Safari → Файл → Добавить в Dock. Затем нажмите «Обновить».",
                      )}
                </p>
              </Card>
            )}
            {config.music_service !== "youtube_music" && (
              <div className="help-note">
                <CircleHelp size={17} />
                <p>
                  {t(
                    "Choose a track in the app once to create a queue. Playback requires an available track and, where applicable, a subscription. If the queue is empty, the app opens so you can choose music.",
                    "Один раз выберите трек в приложении, чтобы создать очередь. Нужен доступный трек и, где требуется, подписка. При пустой очереди плеер откроется для выбора музыки.",
                  )}
                </p>
              </div>
            )}
            <Card title={t("Playback behavior", "Как управлять музыкой")}>
              <SwitchRow
                checked={config.follow_playing}
                onChange={(follow_playing) => change({ follow_playing })}
                title={t(
                  "Follow the player that is already playing",
                  "Управлять плеером, который уже играет",
                )}
                detail={t(
                  "Works with these three apps. Otherwise use your preferred service. Disable to always control only the selected app.",
                  "Работает с этими тремя приложениями. Если ничего не играет, используется выбранный сервис. Выключите, чтобы всегда управлять только им.",
                )}
              />
              <div className="mt-4 flex flex-wrap items-center gap-3">
                <Button
                  disabled={busy || dirty || !advanced || !status.accessibility}
                  onClick={() =>
                    void run("mac_controls_action", { liked: false })
                  }
                >
                  <Headphones size={14} />
                  {t("Test play / pause", "Проверить воспроизведение / паузу")}
                </Button>
                {config.music_service === "youtube_music" && (
                  <Button
                    disabled={
                      busy || dirty || !advanced || !status.accessibility
                    }
                    onClick={() =>
                      void run("mac_controls_action", { liked: true })
                    }
                  >
                    {t("Play Liked Music", "Включить «Мне понравилось»")}
                  </Button>
                )}
              </div>
              <p role="status" className="mt-4 text-xs text-fg-2">
                {t(status.message)}
              </p>
            </Card>
          </>
        )}
        {section === "profiles" && (
          <>
            <div className="section-intro">
              <Layers size={22} />
              <div>
                <h2>
                  {t(
                    "The right controls. In every app.",
                    "Свои действия для каждого приложения.",
                  )}
                </h2>
                <p>
                  {t(
                    "Profiles follow the frontmost app. All other apps use your default gestures.",
                    "Профиль включается, когда приложение активно. В остальных действуют основные настройки.",
                  )}
                </p>
              </div>
            </div>
            {!advanced && (
              <p className="help-note">
                {t(
                  "Enable Custom gestures & music on the Knob tab to activate profiles.",
                  "Включите «Жесты и музыка» на вкладке «Колёсико», чтобы профили работали.",
                )}
              </p>
            )}
            <div className="profiles-layout">
              <div className="profile-list">
                <button
                  className={!selected ? "profile-item active" : "profile-item"}
                  onClick={() => setSelected(null)}
                >
                  <Settings2 size={17} />
                  <span>
                    {t("Default", "Основной")}
                    <small>
                      {t("All other apps", "Все остальные приложения")}
                    </small>
                  </span>
                </button>
                {config.profiles.map((p) => (
                  <button
                    key={p.id}
                    className={`profile-item ${selected === p.id ? "active" : ""}`}
                    onClick={() => setSelected(p.id)}
                  >
                    <Layers size={17} />
                    <span>
                      {p.name}
                      <small>
                        {p.enabled
                          ? t("Automatic", "Автоматически")
                          : t("Disabled", "Выключен")}
                      </small>
                    </span>
                    {p.id === status.active_profile && (
                      <span className="h-1.5 w-1.5 rounded-full bg-good" />
                    )}
                  </button>
                ))}
                <button
                  className="profile-add"
                  disabled={config.profiles.length >= 32}
                  onClick={() => setAdding(!adding)}
                >
                  <Plus size={15} />
                  {t("Add app", "Добавить приложение")}
                </button>
              </div>
              <div className="space-y-4">
                {adding && (
                  <Card title={t("Choose an app", "Выберите приложение")}>
                    <input
                      className={`${INPUT} mb-3 w-full`}
                      aria-label={t("Find app", "Найти приложение")}
                      placeholder={t(
                        "Search installed apps…",
                        "Поиск установленных приложений…",
                      )}
                      value={search}
                      onChange={(e) => setSearch(e.target.value)}
                    />
                    <select
                      className={`${INPUT} mb-4 w-full`}
                      aria-label={t("Application", "Приложение")}
                      value={appChoice}
                      onChange={(e) => setAppChoice(e.target.value)}
                    >
                      <option value="">
                        {t(
                          "Select an installed app",
                          "Выберите установленное приложение",
                        )}
                      </option>
                      {availableApps.map((a) => (
                        <option key={a.id} value={a.id}>
                          {a.name}
                        </option>
                      ))}
                    </select>
                    <div className="flex gap-2">
                      <Button
                        variant="primary"
                        disabled={!appChoice}
                        onClick={addProfile}
                      >
                        {t("Add profile", "Добавить профиль")}
                      </Button>
                      <Button onClick={() => setAdding(false)}>
                        {t("Cancel")}
                      </Button>
                    </div>
                  </Card>
                )}
                {profile ? (
                  <>
                    <Card
                      title={profile.name}
                      action={
                        <Button
                          size="sm"
                          onClick={() => {
                            change({
                              profiles: config.profiles.filter(
                                (p) => p.id !== selected,
                              ),
                            });
                            setSelected(null);
                          }}
                        >
                          <Trash2 size={13} />
                          {t("Remove")}
                        </Button>
                      }
                    >
                      <label className="mb-4 block">
                        <span className="mb-2 block text-xs text-fg-2">
                          {t("Profile name", "Название профиля")}
                        </span>
                        <input
                          className={`${INPUT} w-full`}
                          value={profile.name}
                          maxLength={100}
                          onChange={(e) =>
                            profileChange({ name: e.target.value })
                          }
                        />
                      </label>
                      <SwitchRow
                        checked={profile.enabled}
                        onChange={(enabled) => profileChange({ enabled })}
                        title={t(
                          "Switch automatically",
                          "Включать автоматически",
                        )}
                        detail={profile.bundle_id}
                      />
                      <BindingRows
                        value={profile.bindings}
                        onChange={(bindings) => profileChange({ bindings })}
                        rotation={config.capture_rotation}
                      />
                    </Card>
                    <Card
                      title={t("Keys in this app", "Клавиши в этом приложении")}
                    >
                      <p className="mb-4 text-sm text-fg-2">
                        {t(
                          "Overrides your base mappings while this app is active. Modder itself always uses the base mappings so you can edit them safely.",
                          "Заменяют основные назначения только в этом приложении. В самом Modder действуют основные — их можно спокойно редактировать.",
                        )}
                      </p>
                      {profile.remaps.map((r, index) => (
                        <div
                          className="mb-3 flex items-center gap-2"
                          key={index}
                        >
                          {(["source", "destination"] as const).map(
                            (field, n) => (
                              <span
                                key={field}
                                className="flex min-w-0 flex-1 items-center gap-2"
                              >
                                {n === 1 && <ChevronRight size={14} />}
                                <select
                                  className={`${INPUT} w-full`}
                                  aria-label={`${t(field === "source" ? "From" : "To", field === "source" ? "Из" : "В")} ${index + 1}`}
                                  value={r[field]}
                                  onChange={(e) =>
                                    profileChange({
                                      remaps: profile.remaps.map((v, i) =>
                                        i === index
                                          ? {
                                              ...v,
                                              [field]: Number(e.target.value),
                                            }
                                          : v,
                                      ),
                                    })
                                  }
                                >
                                  {KEYS.map((k) => (
                                    <option key={k} value={k}>
                                      {hidName(k)}
                                    </option>
                                  ))}
                                </select>
                              </span>
                            ),
                          )}
                          <Button
                            size="sm"
                            onClick={() =>
                              profileChange({
                                remaps: profile.remaps.filter(
                                  (_, i) => i !== index,
                                ),
                              })
                            }
                          >
                            <Trash2 size={13} />
                          </Button>
                        </div>
                      ))}
                      <Button
                        disabled={profile.remaps.length >= 128}
                        onClick={() =>
                          profileChange({
                            remaps: [
                              ...profile.remaps,
                              {
                                source:
                                  KEYS.find(
                                    (k) =>
                                      !profile.remaps.some(
                                        (r) => r.source === k,
                                      ),
                                  ) ?? 4,
                                destination: 41,
                              },
                            ],
                          })
                        }
                      >
                        <Plus size={14} />
                        {t("Add mapping")}
                      </Button>
                    </Card>
                  </>
                ) : (
                  <Card title={t("Your everyday profile", "Основной профиль")}>
                    <div className="empty-profile">
                      <Keyboard size={36} strokeWidth={1} />
                      <p>
                        {t(
                          "A familiar default. Smart exceptions.",
                          "Привычная основа. Удобные исключения.",
                        )}
                      </p>
                      <span>
                        {t(
                          "Add a browser for zoom, an editor for font size, or any app for different key mappings. Your default gestures stay on the Knob tab.",
                          "Добавьте браузер для масштаба, редактор для размера шрифта или любое приложение для своих клавиш. Основные жесты настраиваются во вкладке «Колёсико».",
                        )}
                      </span>
                      <Button onClick={() => setAdding(true)}>
                        <Plus size={14} />
                        {t("Create first profile", "Создать первый профиль")}
                      </Button>
                    </div>
                  </Card>
                )}
              </div>
            </div>
          </>
        )}
        {section === "general" && (
          <>
            <Card title={t("Make yourself at home", "Удобство каждый день")}>
              <label className="setting-row !pt-0">
                <span className="text-sm font-medium">
                  {t("Language", "Язык")}
                </span>
                <select
                  className={INPUT}
                  value={language}
                  aria-label={t("Language", "Язык")}
                  onChange={(e) => setLanguage(e.target.value as "en" | "ru")}
                >
                  <option value="ru">Русский</option>
                  <option value="en">English</option>
                </select>
              </label>
              <SwitchRow
                checked={config.launch_at_login}
                onChange={(launch_at_login) => change({ launch_at_login })}
                title={t("Start with your Mac", "Запускать вместе с Mac")}
                detail={t(
                  "Your controls are ready after you sign in. No Dock icon or open window required.",
                  "Управление доступно сразу после входа. Без открытого окна и значка в Dock.",
                )}
              />
            </Card>
            <Card
              title={t("Quietly in the menu bar", "Работает в строке меню")}
            >
              <p className="text-sm leading-relaxed text-fg-2">
                {t(
                  "Close the window or press ⌘Q — your controls keep working. Reopen settings from AK820 in the menu bar. To restore owned mappings and stop all controls, choose Stop controls and quit there.",
                  "Закройте окно или нажмите ⌘Q — управление продолжит работать. Настройки открываются через AK820 в строке меню. Для полного завершения с восстановлением прежних назначений выберите там Stop controls and quit.",
                )}
              </p>
            </Card>
            <div className="help-note">
              <ShieldCheck size={18} />
              <p>
                {t(
                  "Profiles and gestures are saved on this Mac. Lighting and TFT images/GIFs still use USB; host controls also work over Bluetooth.",
                  "Профили и жесты сохраняются на этом Mac. Подсветка и загрузка GIF на TFT доступны по USB; управление на Mac работает и по Bluetooth.",
                )}
              </p>
            </div>
          </>
        )}
      </div>
      <div className="save-bar">
        <span
          className={`flex items-center gap-2 text-sm ${dirty ? "text-fg-1" : "text-fg-2"}`}
        >
          {dirty ? (
            <>
              <span className="h-1.5 w-1.5 rounded-full bg-warn" />
              {t("You have unsaved changes", "Есть несохранённые изменения")}
            </>
          ) : (
            <>
              <Check size={15} />
              {saved
                ? t("Saved on this Mac.")
                : t("All changes saved", "Все изменения сохранены")}
            </>
          )}
        </span>
        <div className="flex gap-2">
          <Button
            disabled={busy || !dirty}
            onClick={() => {
              setConfig(status.config);
              setError(null);
              setSaved(false);
            }}
          >
            {t("Revert", "Отменить")}
          </Button>
          <Button
            variant="primary"
            disabled={busy || !dirty}
            onClick={() => void save()}
          >
            {busy ? t("Saving…", "Сохраняем…") : t("Save changes", "Сохранить")}
          </Button>
        </div>
      </div>
    </div>
  );
}
export function UsbRequired() {
  return (
    <Card
      title={t(
        "Connect USB for keyboard settings",
        "Подключите USB для настройки клавиатуры",
      )}
    >
      <p className="text-sm text-fg-2">
        {t(
          "Lighting, system settings and TFT image/GIF uploads need USB. Mac controls, key mappings and macros remain available over Bluetooth.",
          "Для подсветки, системных настроек и загрузки GIF на TFT нужен USB. Колёсико, назначения клавиш и макросы на Mac доступны по Bluetooth.",
        )}
      </p>
    </Card>
  );
}
