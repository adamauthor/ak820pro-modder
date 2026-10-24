import { t } from "../i18n";
/**
 * Named per-key colour profiles shown under the paint surface: save the
 * current paint, apply or delete a profile, share profiles as JSON files
 * and cycle through them with a global shortcut.
 */
import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { confirm, open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { Button, ErrorBanner } from "../components/ui";
import type { CustomLedMap, LedColor } from "../types";
import { formatError } from "../errors";

interface ColourProfile {
  name: string;
  map: CustomLedMap;
}

interface Store {
  current: CustomLedMap | null;
  profiles: ColourProfile[];
  active: string | null;
  cycle_shortcut: string;
}

const INPUT =
  "rounded-sm border border-line bg-surface-base px-2 py-1 text-xs text-fg-0 outline-none focus:border-accent-500/60";

interface Props {
  /** The paint currently on screen; saved as a profile. */
  draft: LedColor[];
  /** Called after a profile was written to the keyboard. */
  onApplied: (map: CustomLedMap) => void;
  busy: boolean;
}

export function ColourProfiles({ draft, onApplied, busy }: Props) {
  const [store, setStore] = useState<Store | null>(null);
  const [name, setName] = useState("");
  const [shortcut, setShortcut] = useState("");
  const [working, setWorking] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const reload = useCallback(async () => {
    const s = await invoke<Store>("colour_profiles_get");
    setStore(s);
    setShortcut(s.cycle_shortcut);
    return s;
  }, []);

  useEffect(() => {
    reload().catch((e) => setErr(formatError(e)));
  }, [reload]);

  // The global shortcut applies profiles while this view is open too.
  useEffect(() => {
    const off = listen<string>("colour-profile-applied", async () => {
      const s = await reload();
      const p = s.profiles.find((x) => x.name === s.active);
      if (p) onApplied(p.map);
    });
    return () => {
      void off.then((f) => f());
    };
  }, [reload, onApplied]);

  async function run(action: () => Promise<string | null>) {
    setWorking(true);
    setErr(null);
    setNotice(null);
    try {
      const message = await action();
      await reload();
      if (message) setNotice(message);
    } catch (e) {
      setErr(formatError(e));
    } finally {
      setWorking(false);
    }
  }

  const saveCurrent = () =>
    run(async () => {
      const trimmed = name.trim();
      if (store?.profiles.some((p) => p.name === trimmed)) {
        const ok = await confirm(t(`Replace the profile "${trimmed}"?`, `Заменить профиль «${trimmed}»?`));
        if (!ok) return null;
      }
      await invoke("colour_profile_save", { name: trimmed, map: { leds: draft } });
      setName("");
      return t(`Saved "${trimmed}".`, `Профиль «${trimmed}» сохранён.`);
    });

  const apply = (profile: string) =>
    run(async () => {
      await invoke<string>("colour_profile_apply", { name: profile });
      const p = store?.profiles.find((x) => x.name === profile);
      if (p) onApplied(p.map);
      return null;
    });

  const remove = (profile: string) =>
    run(async () => {
      const ok = await confirm(t(`Delete the profile "${profile}"?`, `Удалить профиль «${profile}»?`));
      if (!ok) return null;
      await invoke("colour_profile_delete", { name: profile });
      return null;
    });

  const exportAll = () =>
    run(async () => {
      const path = await saveDialog({
        defaultPath: "ak820-colours.json",
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!path) return null;
      await invoke("colour_profiles_export", { path, names: [] });
      return t("Profiles exported.", "Профили экспортированы.");
    });

  const importFile = () =>
    run(async () => {
      const path = await openDialog({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
      if (typeof path !== "string") return null;
      const names = await invoke<string[]>("colour_profiles_import", { path });
      return t(`Imported: ${names.join(", ")}.`, `Импортировано: ${names.join(", ")}.`);
    });

  const saveShortcut = () =>
    run(async () => {
      await invoke("colour_cycle_shortcut_set", { shortcut });
      return shortcut.trim()
        ? t("Shortcut saved. It works while the app is running.", "Сочетание сохранено. Работает, пока приложение запущено.")
        : t("Shortcut turned off.", "Сочетание отключено.");
    });

  const disabled = busy || working;

  return (
    <div className="mt-4 border-t border-line/60 pt-4">
      <div className="mb-3 flex items-center justify-between">
        <span className="kicker">{t("Colour profiles", "Цветовые профили")}</span>
        <div className="flex gap-2">
          <Button size="sm" variant="ghost" onClick={() => void importFile()} disabled={disabled}>
            {t("Import…", "Импорт…")}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => void exportAll()}
            disabled={disabled || !store?.profiles.length}
          >
            {t("Export…", "Экспорт…")}
          </Button>
        </div>
      </div>
      <ErrorBanner>{err}</ErrorBanner>
      {notice && <p className="mb-2 text-xs text-fg-2">{notice}</p>}

      <div className="mb-3 flex flex-wrap items-center gap-2">
        <input
          type="text"
          className={`${INPUT} w-48`}
          maxLength={40}
          placeholder={t("Profile name", "Название профиля")}
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && name.trim()) void saveCurrent();
          }}
        />
        <Button size="sm" variant="primary" onClick={() => void saveCurrent()} disabled={disabled || !name.trim()}>
          {t("Save current paint", "Сохранить раскраску")}
        </Button>
      </div>

      {store && store.profiles.length > 0 ? (
        <ul className="mb-3 grid gap-1.5">
          {store.profiles.map((p) => (
            <li
              key={p.name}
              className="flex items-center gap-3 rounded-md border border-line/60 bg-surface-elevated/40 px-3 py-1.5"
            >
              <Swatches map={p.map} />
              <span className="flex-1 truncate text-sm">
                {p.name}
                {store.active === p.name && (
                  <span className="ml-2 text-xs text-fg-3">{t("active", "активен")}</span>
                )}
              </span>
              <Button size="sm" variant="ghost" onClick={() => void apply(p.name)} disabled={disabled}>
                {t("Apply", "Применить")}
              </Button>
              <Button size="sm" variant="ghost" onClick={() => void remove(p.name)} disabled={disabled}>
                {t("Delete", "Удалить")}
              </Button>
            </li>
          ))}
        </ul>
      ) : (
        <p className="mb-3 text-xs text-fg-3">
          {t("No saved profiles yet.", "Сохранённых профилей пока нет.")}
        </p>
      )}

      <div className="flex flex-wrap items-center gap-2">
        <span className="text-xs text-fg-2">{t("Next profile shortcut", "Сочетание «следующий профиль»")}</span>
        <input
          type="text"
          className={`${INPUT} w-44`}
          placeholder={t("Control+Alt+C")}
          value={shortcut}
          onChange={(e) => setShortcut(e.target.value)}
        />
        <Button
          size="sm"
          variant="ghost"
          onClick={() => void saveShortcut()}
          disabled={disabled || shortcut.trim() === (store?.cycle_shortcut ?? "")}
        >
          {t("Save", "Сохранить")}
        </Button>
      </div>
    </div>
  );
}

/** A few distinct colours from the map, as a compact preview. */
function Swatches({ map }: { map: CustomLedMap }) {
  const seen = new Set<string>();
  for (const l of map.leds) {
    if (l.red + l.green + l.blue === 0) continue;
    seen.add(`rgb(${l.red},${l.green},${l.blue})`);
    if (seen.size >= 5) break;
  }
  const colours = [...seen];
  return (
    <span className="flex gap-0.5">
      {(colours.length ? colours : ["rgb(20,22,26)"]).map((c) => (
        <span key={c} className="h-3.5 w-3.5 rounded-[3px] border border-line/70" style={{ backgroundColor: c }} />
      ))}
    </span>
  );
}
