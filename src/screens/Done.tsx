import { useState, useEffect } from "react";
import { ipc, FolderStats } from "../ipc";
import { CheckCircle2, FolderOpen, Copy, RefreshCw, ArrowLeft } from "lucide-react";

interface Props {
  folderPath: string;
  onNewFolder: () => void;
}

export function DoneScreen({ folderPath, onNewFolder }: Props) {
  const [stats, setStats] = useState<FolderStats | null>(null);
  const [copied, setCopied] = useState(false);
  const [regenerating, setRegenerating] = useState(false);

  useEffect(() => {
    ipc.getFolderStats(folderPath).then(setStats).catch((e) => console.error(e));
  }, [folderPath]);

  async function copyIndex() {
    try {
      const content = await ipc.readIndexFile(folderPath);
      await navigator.clipboard.writeText(content);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (e) {
      console.error("Failed to copy index:", e);
    }
  }

  async function regenerate() {
    setRegenerating(true);
    try {
      await ipc.regenerateIndex(folderPath);
    } catch (e) {
      console.error("Failed to regenerate:", e);
    } finally {
      setRegenerating(false);
    }
  }

  return (
    <div className="flex-1 flex flex-col items-center justify-center p-8">
      <div
        className="w-full max-w-lg rounded-xl p-8"
        style={{ background: "var(--bg-card)", border: "1px solid var(--border)" }}
      >
        {/* Header */}
        <div className="flex items-center gap-3 mb-6">
          <CheckCircle2 size={32} style={{ color: "var(--success)" }} />
          <div>
            <h2 className="text-xl font-bold">Processing Complete</h2>
            <p className="text-sm" style={{ color: "var(--text-muted)" }}>
              {folderPath.split("/").pop() || folderPath.split("\\").pop()}
            </p>
          </div>
        </div>

        {/* Stats */}
        {stats && (
          <div className="grid grid-cols-2 gap-4 mb-6">
            <StatCard label="Described" value={String(stats.done)} color="var(--success)" />
            <StatCard label="Queued" value={String(stats.queued)} color="var(--text-muted)" />
          </div>
        )}

        {/* Index */}
        <div
          className="rounded-lg p-3 mb-4 flex items-center justify-between"
          style={{ background: "var(--bg-secondary)" }}
        >
          <span className="flex items-center gap-1 text-sm" style={{ color: "var(--success)" }}>
            <CheckCircle2 size={14} />
            INDEX.md written
          </span>
          <button
            onClick={regenerate}
            disabled={regenerating}
            className="text-xs px-2 py-1 rounded hover:opacity-70 flex items-center gap-1"
            style={{ color: "var(--accent)" }}
          >
            <RefreshCw size={12} className={regenerating ? "animate-spin" : ""} />
            Regenerate
          </button>
        </div>

        {/* Actions */}
        <div className="space-y-2">
          <button
            onClick={copyIndex}
            className="w-full flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg text-sm font-medium text-white transition-colors"
            style={{ background: "var(--accent)" }}
          >
            <Copy size={14} />
            {copied ? "Copied!" : "Copy INDEX.md to Clipboard"}
          </button>

          <div className="flex gap-2">
            <button
              onClick={onNewFolder}
              className="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg text-sm font-medium transition-colors"
              style={{ background: "var(--bg-secondary)", color: "var(--text)" }}
            >
              <ArrowLeft size={14} />
              New Folder
            </button>
            <button
              onClick={async () => {
                const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
                revealItemInDir(folderPath);
              }}
              className="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg text-sm font-medium transition-colors"
              style={{ background: "var(--bg-secondary)", color: "var(--text)" }}
            >
              <FolderOpen size={14} />
              Reveal in Finder
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

function StatCard({ label, value, color }: { label: string; value: string; color: string }) {
  return (
    <div className="rounded-lg p-3 text-center" style={{ background: "var(--bg-secondary)" }}>
      <div className="text-2xl font-bold" style={{ color }}>{value}</div>
      <div className="text-xs" style={{ color: "var(--text-muted)" }}>{label}</div>
    </div>
  );
}
