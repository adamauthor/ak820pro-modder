import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Badge, Button, Card, ErrorBanner } from "../components/ui";
import { PageHeader } from "../components/Layout";
import { formatError } from "../errors";
import { t, useI18n } from "../i18n";

type Part = "lighting" | "knob";
interface Preset {
  id: string;
  icon: string;
  name: string;
  name_ru: string;
  description: string;
  description_ru: string;
  lighting: { color: string; mode: string } | null;
  knob: { single: string; double: string; triple: string } | null;
}
interface Report { applied: Part[]; warnings: string[] }

const PARTS: [Part, () => string][] = [
  ["lighting", () => t("Lighting", "Подсветка")],
  ["knob", () => t("Knob", "Колёсико")],
];

/** One-click setups that combine lighting and knob gestures. Each part can
 *  be left out. */
export function Presets() {
  const { language } = useI18n();
  const ru = language === "ru";
  const [presets, setPresets] = useState<Preset[]>([]);
  const [selected, setSelected] = useState<Record<string, Record<Part, boolean>>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [report, setReport] = useState<{ id: string; report: Report } | null>(null);

  useEffect(() => {
    invoke<Preset[]>("list_presets")
      .then(list => {
        setPresets(list);
        setSelected(Object.fromEntries(list.map(p => [p.id, Object.fromEntries(PARTS.map(([part]) => [part, p[part] !== null])) as Record<Part, boolean>])));
      })
      .catch(e => setError(formatError(e)));
  }, []);

  async function apply(preset: Preset) {
    setBusy(preset.id); setError(null); setReport(null);
    try {
      const result = await invoke<Report>("apply_preset", { id: preset.id, options: selected[preset.id] });
      setReport({ id: preset.id, report: result });
      window.dispatchEvent(new Event("mac-controls-updated"));
    } catch (e) { setError(formatError(e)); }
    finally { setBusy(null); }
  }

  return <div className="space-y-5">
    <PageHeader title={t("Presets", "Пресеты")}
      description={t("Set the lighting and knob for a situation in one click. Your music service, app profiles and key mappings are never changed.",
        "Подсветка и колёсико под ситуацию одним кликом. Музыкальный сервис, профили приложений и переназначения клавиш не меняются.")} />
    <ErrorBanner>{error}</ErrorBanner>
    <div className="grid gap-4 md:grid-cols-2">
      {presets.map(preset => <Card key={preset.id} title={<span className="flex items-center gap-2"><span className="text-xl">{preset.icon}</span>{ru ? preset.name_ru : preset.name}</span>}
        action={preset.lighting && <span className="inline-block h-4 w-4 rounded-full border border-line" style={{ background: `#${preset.lighting.color}` }} />}>
        <p className="mb-4 text-sm text-fg-2">{ru ? preset.description_ru : preset.description}</p>
        <div className="mb-4 flex flex-wrap gap-x-4 gap-y-2 text-sm">
          {PARTS.filter(([part]) => preset[part] !== null).map(([part, label]) => <label key={part} className="flex items-center gap-2">
            <input type="checkbox" checked={selected[preset.id]?.[part] ?? false}
              onChange={e => setSelected({ ...selected, [preset.id]: { ...selected[preset.id], [part]: e.target.checked } })} />{label()}
          </label>)}
        </div>
        <Button variant="primary" disabled={busy !== null || !Object.values(selected[preset.id] ?? {}).some(Boolean)} onClick={() => void apply(preset)}>
          {busy === preset.id ? t("Applying…", "Применяем…") : t("Apply", "Применить")}</Button>
        {report?.id === preset.id && <div className="mt-3 space-y-1 text-sm">
          <p className="text-good">{t("Applied", "Применено")}: {report.report.applied.map(part => PARTS.find(([p]) => p === part)?.[1]()).join(", ") || "—"}</p>
          {report.report.warnings.map(w => <p key={w} className="text-warn">{t(w)}</p>)}
        </div>}
        {preset.knob?.single.includes("mute") && <Badge tone="warn" className="mt-3">{t("Click mutes the microphone", "Клик выключает микрофон")}</Badge>}
      </Card>)}
    </div>
  </div>;
}
