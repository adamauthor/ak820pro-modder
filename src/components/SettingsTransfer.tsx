import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { Button, Card, ErrorBanner } from "./ui";
import { formatError } from "../errors";
import { t } from "../i18n";

/** Exports or imports every setting (knob, mappings, macros, automations,
 *  smart lighting, Pomodoro, anime screen) as one JSON file. */
export function SettingsTransfer() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState("");
  async function run(action: "export" | "import") {
    setError(null); setNotice("");
    const filters = [{ name: "AK820 settings", extensions: ["json"] }];
    const path = action === "export"
      ? await save({ defaultPath: "ak820-settings.json", filters })
      : await open({ multiple: false, directory: false, filters });
    if (typeof path !== "string") return;
    setBusy(true);
    try {
      await invoke(action === "export" ? "settings_export" : "settings_import", { path });
      setNotice(action === "export" ? t("Settings exported.", "Настройки экспортированы.") : t("Settings imported and applied.", "Настройки импортированы и применены."));
      if (action === "import") window.dispatchEvent(new Event("mac-controls-updated"));
    } catch (e) { setError(formatError(e)); }
    finally { setBusy(false); }
  }
  return <Card title={t("Settings backup", "Резервная копия настроек")}>
    <p className="mb-4 text-sm text-fg-2">{t("One file with the knob, Mac key mappings, macros, automations, smart lighting, Pomodoro and anime screen settings. Use it to move to another Mac.",
      "Один файл с настройками колёсика, переназначений, макросов, автоматизаций, умной подсветки, помодоро и аниме-экрана. Удобно для переноса на другой Mac.")}</p>
    <ErrorBanner>{error}</ErrorBanner>
    {notice && <p role="status" className="mb-3 text-sm text-good">{notice}</p>}
    <div className="flex flex-wrap gap-2">
      <Button disabled={busy} onClick={() => void run("export")}>{t("Export…", "Экспорт…")}</Button>
      <Button disabled={busy} onClick={() => void run("import")}>{t("Import…", "Импорт…")}</Button>
    </div>
  </Card>;
}
