import { useState, useEffect } from "react";
import { ipc, Settings, SettingsView } from "../ipc";
import { ArrowLeft, Save, Loader2, ChevronDown, ChevronRight } from "lucide-react";

interface Props {
  onBack: () => void;
}

function Section({
  title,
  defaultOpen = true,
  children,
}: {
  title: string;
  defaultOpen?: boolean;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div
      className="rounded-lg overflow-hidden"
      style={{ border: "1px solid var(--border)", background: "var(--bg-card)" }}
    >
      <button
        onClick={() => setOpen(!open)}
        className="w-full flex items-center gap-2 px-4 py-3 text-sm font-semibold text-left hover:opacity-80 transition-opacity"
      >
        {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        {title}
      </button>
      {open && <div className="px-4 pb-4 space-y-4">{children}</div>}
    </div>
  );
}

export function SettingsScreen({ onBack }: Props) {
  const [view, setView] = useState<SettingsView | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [apiKeyTouched, setApiKeyTouched] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    ipc.getSettings().then(setView).catch((e) => console.error("Failed to load settings:", e));
  }, []);

  async function save() {
    if (!view) return;
    setSaving(true);
    setError(null);
    try {
      const settings: Settings = {
        api_key: apiKeyTouched ? apiKey : "",
        model: view.model,
        media_resolution: view.media_resolution,
        concurrency: view.concurrency,
        overwrite_policy: view.overwrite_policy,
        custom_prompt: view.custom_prompt,
        split_threshold_gb: view.split_threshold_gb,
        segment_duration_min: view.segment_duration_min,
        include_transcript: view.include_transcript,
      };
      await ipc.saveSettings(settings);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
      const updated = await ipc.getSettings();
      setView(updated);
      setApiKeyTouched(false);
      setApiKey("");
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  if (!view) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="animate-spin" />
      </div>
    );
  }

  const updateView = (key: keyof SettingsView, value: string | number | boolean) => {
    setView({ ...view, [key]: value });
    setSaved(false);
  };

  const inputStyle = {
    background: "var(--bg-secondary)",
    border: "1px solid var(--border)",
    color: "var(--text)",
  };

  return (
    <div className="flex flex-col min-h-screen" style={{ background: "var(--bg)" }}>
      {/* Sticky header */}
      <div
        className="sticky top-0 z-10 flex items-center gap-3 px-4 py-3 border-b"
        style={{ borderColor: "var(--border)", background: "var(--bg-card)" }}
      >
        <button onClick={onBack} className="p-1 rounded hover:opacity-70">
          <ArrowLeft size={18} />
        </button>
        <h2 className="text-lg font-semibold flex-1">Settings</h2>
        <button
          onClick={save}
          disabled={saving}
          className="flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium text-white transition-colors"
          style={{ background: "var(--accent)" }}
        >
          {saving ? <Loader2 size={14} className="animate-spin" /> : <Save size={14} />}
          {saved ? "Saved!" : "Save"}
        </button>
      </div>

      {/* Scrollable content that sizes to fit */}
      <div className="overflow-y-auto p-4 pb-8">
        <div className="max-w-2xl mx-auto space-y-4">
          {/* Error */}
          {error && (
            <div className="p-3 rounded-lg text-sm" style={{ background: "#fee2e2", color: "#991b1b" }}>
              {error}
            </div>
          )}

          {/* ── API & Account ── */}
          <Section title="API & Account">
            <Field label="Gemini API Key" description="">
              <p className="text-xs mb-1.5" style={{ color: "var(--text-muted)" }}>
                Free — no credit card needed.{" "}
                <a
                  href="https://aistudio.google.com/apikey"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="underline"
                  style={{ color: "var(--accent)" }}
                >
                  Get your key from Google AI Studio →
                </a>
              </p>
              {view.api_key_set && !apiKeyTouched ? (
                <div className="flex items-center gap-2">
                  <span className="text-sm font-mono" style={{ color: "var(--text-muted)" }}>
                    {view.api_key_preview}
                  </span>
                  <button
                    onClick={() => { setApiKeyTouched(true); setApiKey(""); }}
                    className="text-xs px-2 py-1 rounded"
                    style={{ color: "var(--accent)" }}
                  >
                    Change
                  </button>
                </div>
              ) : (
                <input
                  type="password"
                  value={apiKey}
                  onChange={(e) => { setApiKey(e.target.value); setApiKeyTouched(true); }}
                  placeholder="AIza..."
                  className="w-full px-3 py-2 rounded-lg text-sm outline-none"
                  style={inputStyle}
                />
              )}
            </Field>

            <div className="p-3 rounded-lg text-xs" style={{ background: "var(--bg-secondary)", color: "var(--text-muted)" }}>
              Free tier keys may allow Google to use submitted content for training.
              To exclude your data, enable billing on your Google Cloud account.
            </div>
          </Section>

          {/* ── Model & Quality ── */}
          <Section title="Model & Quality">
            <Field label="Model" description="Choose quality vs cost tradeoff">
              <select
                value={view.model}
                onChange={(e) => updateView("model", e.target.value)}
                className="w-full px-3 py-2 rounded-lg text-sm outline-none"
                style={inputStyle}
              >
                <option value="gemini-2.5-flash-lite">Gemini 2.5 Flash-Lite — cheapest</option>
                <option value="gemini-3.1-flash-lite-preview">Gemini 3.1 Flash-Lite — default</option>
                <option value="gemini-3-flash-preview">Gemini 3 Flash — richest</option>
              </select>
            </Field>

            <Field label="Media Resolution" description="Low is recommended for most footage">
              <select
                value={view.media_resolution}
                onChange={(e) => updateView("media_resolution", e.target.value)}
                className="w-full px-3 py-2 rounded-lg text-sm outline-none"
                style={inputStyle}
              >
                <option value="low">Low (recommended)</option>
                <option value="high">High (for text-heavy footage)</option>
              </select>
            </Field>

            <label className="flex items-center gap-3 cursor-pointer pt-1">
              <input
                type="checkbox"
                checked={view.include_transcript}
                onChange={(e) => updateView("include_transcript", e.target.checked)}
                className="w-4 h-4 rounded"
              />
              <div>
                <span className="text-sm font-medium">Include transcript</span>
                <p className="text-xs" style={{ color: "var(--text-muted)" }}>
                  Verbatim speech transcription via Gemini (increases output tokens)
                </p>
              </div>
            </label>
          </Section>

          {/* ── Processing ── */}
          <Section title="Processing">
            <div className="grid grid-cols-2 gap-4">
              <Field label="Concurrency" description="Parallel workers (1-10)">
                <input
                  type="number"
                  min={1}
                  max={10}
                  value={view.concurrency}
                  onChange={(e) => updateView("concurrency", Number(e.target.value))}
                  className="w-full px-3 py-2 rounded-lg text-sm outline-none"
                  style={inputStyle}
                />
              </Field>

              <Field label="Overwrite Policy" description="Existing sidecars">
                <select
                  value={view.overwrite_policy}
                  onChange={(e) => updateView("overwrite_policy", e.target.value)}
                  className="w-full px-3 py-2 rounded-lg text-sm outline-none"
                  style={inputStyle}
                >
                  <option value="skip">Skip</option>
                  <option value="overwrite">Overwrite</option>
                </select>
              </Field>
            </div>
          </Section>

          {/* ── Large Files ── */}
          <Section title="Large File Splitting" defaultOpen={false}>
            <p className="text-xs" style={{ color: "var(--text-muted)" }}>
              Files exceeding the threshold are split and compressed to 480p before upload. Each segment is analysed separately.
            </p>
            <div className="grid grid-cols-2 gap-4">
              <Field label="Split Threshold" description="">
                <div className="flex items-center gap-2">
                  <input
                    type="number"
                    min={0.5}
                    max={100}
                    step={0.5}
                    value={view.split_threshold_gb}
                    onChange={(e) => updateView("split_threshold_gb", Number(e.target.value))}
                    className="w-full px-3 py-2 rounded-lg text-sm outline-none"
                    style={inputStyle}
                  />
                  <span className="text-sm shrink-0" style={{ color: "var(--text-muted)" }}>GB</span>
                </div>
              </Field>

              <Field label="Segment Duration" description="">
                <div className="flex items-center gap-2">
                  <input
                    type="number"
                    min={1}
                    max={120}
                    step={5}
                    value={view.segment_duration_min}
                    onChange={(e) => updateView("segment_duration_min", Number(e.target.value))}
                    className="w-full px-3 py-2 rounded-lg text-sm outline-none"
                    style={inputStyle}
                  />
                  <span className="text-sm shrink-0" style={{ color: "var(--text-muted)" }}>min</span>
                </div>
              </Field>
            </div>
          </Section>

          {/* ── Advanced ── */}
          <Section title="Advanced" defaultOpen={false}>
            <Field label="Custom Prompt" description="Appended to the system prompt for all clips (max 2000 chars)">
              <textarea
                value={view.custom_prompt}
                onChange={(e) => updateView("custom_prompt", e.target.value)}
                placeholder="e.g., Focus on identifying bird species..."
                rows={3}
                maxLength={2000}
                className="w-full px-3 py-2 rounded-lg text-sm outline-none resize-y"
                style={inputStyle}
              />
            </Field>
          </Section>
        </div>
      </div>
    </div>
  );
}

function Field({
  label,
  description,
  children,
}: {
  label: string;
  description: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <label className="block text-sm font-medium mb-0.5">{label}</label>
      {description && (
        <p className="text-xs mb-1.5" style={{ color: "var(--text-muted)" }}>
          {description}
        </p>
      )}
      {children}
    </div>
  );
}
