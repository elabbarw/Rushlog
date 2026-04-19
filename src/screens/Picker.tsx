import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { ipc, ScanResult } from "../ipc";
import { FolderOpen, Settings, Info, Loader2 } from "lucide-react";

interface Props {
  onFolderScanned: (result: ScanResult) => void;
  onOpenSettings: () => void;
}

export function PickerScreen({ onFolderScanned, onOpenSettings }: Props) {
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showInfo, setShowInfo] = useState(false);

  async function pickFolder() {
    const selected = await open({ directory: true, multiple: false });
    if (!selected) return;
    await scanFolder(selected as string);
  }

  async function scanFolder(path: string) {
    setScanning(true);
    setError(null);
    try {
      const result = await ipc.scanFolder(path);
      onFolderScanned(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setScanning(false);
    }
  }

  async function handleDrop(e: React.DragEvent) {
    e.preventDefault();
    const items = e.dataTransfer.items;
    if (items.length > 0) {
      const item = items[0];
      if (item.kind === "file") {
        const entry = item.webkitGetAsEntry?.();
        if (entry?.isDirectory) {
          // For Tauri, we need the path from the file
          const file = e.dataTransfer.files[0];
          if (file) {
            // Tauri drag and drop provides the path
            const path = (file as any).path || file.name;
            await scanFolder(path);
          }
        }
      }
    }
  }

  return (
    <div className="flex-1 flex flex-col items-center justify-center p-8">
      {/* Settings gear */}
      <button
        onClick={onOpenSettings}
        className="absolute top-4 right-4 p-2 rounded-lg hover:opacity-70 transition-opacity"
        style={{ color: "var(--text-muted)" }}
        title="Settings"
      >
        <Settings size={20} />
      </button>

      {/* Logo / Title */}
      <div className="mb-8 text-center">
        <h1 className="text-3xl font-bold tracking-tight mb-2">Rushlog</h1>
        <p style={{ color: "var(--text-muted)" }} className="text-sm">
          Turn a folder of video rushes into searchable markdown
        </p>
      </div>

      {/* Drop zone */}
      <div
        onDragOver={(e) => e.preventDefault()}
        onDrop={handleDrop}
        onClick={pickFolder}
        className="w-full max-w-md rounded-xl border-2 border-dashed p-12 flex flex-col items-center justify-center cursor-pointer transition-all hover:border-[var(--accent)]"
        style={{
          borderColor: "var(--border)",
          background: "var(--bg-card)",
        }}
      >
        {scanning ? (
          <Loader2 size={48} className="animate-spin mb-4" style={{ color: "var(--accent)" }} />
        ) : (
          <FolderOpen size={48} className="mb-4" style={{ color: "var(--text-muted)" }} />
        )}
        <p className="font-medium mb-1">
          {scanning ? "Scanning folder..." : "Drop a folder here"}
        </p>
        <p className="text-sm" style={{ color: "var(--text-muted)" }}>
          or click to choose
        </p>
      </div>

      {error && (
        <div
          className="mt-4 p-3 rounded-lg text-sm max-w-md w-full"
          style={{ background: "var(--bg-secondary)", color: "var(--error)" }}
        >
          {error}
        </div>
      )}

      {/* Info link */}
      <button
        onClick={() => setShowInfo(!showInfo)}
        className="mt-6 flex items-center gap-1 text-sm hover:underline"
        style={{ color: "var(--text-muted)" }}
      >
        <Info size={14} />
        What gets sent to Google?
      </button>

      {showInfo && (
        <div
          className="mt-3 p-4 rounded-lg text-sm max-w-md w-full"
          style={{ background: "var(--bg-card)", border: "1px solid var(--border)" }}
        >
          <p className="mb-2">
            <strong>Video files</strong> are uploaded to Google's Gemini API for analysis.
            Google processes the visual and audio content to generate descriptions.
          </p>
          <p className="mb-2">
            Files are stored on Google's servers for up to 48 hours, then deleted.
            No other data leaves your machine.
          </p>
          <p style={{ color: "var(--warning)" }}>
            <strong>Free tier:</strong> Google may use submitted content to improve their models.
            Use a paid API key for confidential footage.
          </p>
        </div>
      )}
    </div>
  );
}
