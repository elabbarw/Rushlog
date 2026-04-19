import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { ipc, ScanResult, ClipInfo, ProgressEvent } from "../ipc";
import { ArrowLeft, Play, Loader2, AlertCircle, CheckCircle2, Clock, Film } from "lucide-react";

interface Props {
  scanResult: ScanResult;
  onComplete: () => void;
  onBack: () => void;
}

interface ClipState extends ClipInfo {
  error?: string;
  statusDetail?: string; // e.g. "uploading 2/6", "describing", "splitting"
}

function formatSize(bytes: number): string {
  if (!bytes) return "";
  if (bytes > 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
  if (bytes > 1e6) return `${(bytes / 1e6).toFixed(0)} MB`;
  return `${(bytes / 1e3).toFixed(0)} KB`;
}

function StatusPill({ status }: { status: string }) {
  const colors: Record<string, { bg: string; text: string }> = {
    queued: { bg: "#e0e7ff", text: "#3730a3" },
    processing: { bg: "#fef3c7", text: "#92400e" },
    done: { bg: "#dcfce7", text: "#166534" },
    error: { bg: "#fee2e2", text: "#991b1b" },
  };
  const c = colors[status] || colors.queued;
  return (
    <span
      className="text-xs font-medium px-2 py-0.5 rounded-full"
      style={{ background: c.bg, color: c.text }}
    >
      {status}
    </span>
  );
}

export function QueueScreen({ scanResult, onComplete, onBack }: Props) {
  const [clips, setClips] = useState<ClipState[]>(scanResult.clips);
  const [processing, setProcessing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [doneCount, setDoneCount] = useState(
    scanResult.clips.filter((c) => c.status === "done").length
  );
  const [errorCount, setErrorCount] = useState(0);

  const totalClips = clips.length;
  const queuedCount = clips.filter((c) => c.status === "queued").length;

  useEffect(() => {
    const unlisten1 = listen<ProgressEvent>("processing-progress", (event) => {
      const data = event.payload;
      if (data.type === "ClipDone") {
        setClips((prev) =>
          prev.map((c) =>
            c.filename === data.filename
              ? { ...c, status: "done", statusDetail: undefined }
              : c
          )
        );
        setDoneCount((n) => n + 1);
      } else if (data.type === "ClipError") {
        setClips((prev) =>
          prev.map((c) =>
            c.filename === data.filename
              ? { ...c, status: "error", error: data.error, statusDetail: undefined }
              : c
          )
        );
        setErrorCount((n) => n + 1);
      } else if (data.type === "ClipStatus") {
        setClips((prev) =>
          prev.map((c) =>
            c.filename === data.filename
              ? { ...c, status: "processing", statusDetail: data.status }
              : c
          )
        );
      } else if (data.type === "Complete") {
        setProcessing(false);
        setTimeout(onComplete, 500);
      }
    });

    const unlisten2 = listen<boolean>("processing-complete", () => {
      setProcessing(false);
    });

    return () => {
      unlisten1.then((f) => f());
      unlisten2.then((f) => f());
    };
  }, [onComplete]);

  async function startProcessing() {
    setError(null);
    try {
      setProcessing(true);
      setClips((prev) =>
        prev.map((c) => (c.status === "queued" ? { ...c, status: "processing" } : c))
      );
      await ipc.startProcessing(scanResult.folder_path);
    } catch (e) {
      setError(String(e));
      setProcessing(false);
    }
  }

  const progressPct = totalClips > 0 ? ((doneCount + errorCount) / totalClips) * 100 : 0;

  return (
    <div className="flex-1 flex flex-col" style={{ background: "var(--bg)" }}>
      {/* Top bar */}
      <div
        className="flex items-center gap-3 px-4 py-3 border-b"
        style={{ borderColor: "var(--border)", background: "var(--bg-card)" }}
      >
        <button onClick={onBack} className="p-1 rounded hover:opacity-70" title="Back">
          <ArrowLeft size={18} />
        </button>

        <div className="flex-1 min-w-0">
          <div className="text-sm font-medium truncate">
            {scanResult.folder_path.split("/").pop() || scanResult.folder_path.split("\\").pop()}
          </div>
          <div className="text-xs" style={{ color: "var(--text-muted)" }}>
            {doneCount} / {totalClips} done
            {errorCount > 0 && <span style={{ color: "var(--error)" }}> · {errorCount} errors</span>}
            {queuedCount > 0 && ` · ${queuedCount} queued`}
          </div>
        </div>

        {!processing ? (
          <button
            onClick={startProcessing}
            disabled={queuedCount === 0}
            className="flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium text-white disabled:opacity-50 transition-colors"
            style={{ background: queuedCount > 0 ? "var(--accent)" : "var(--text-muted)" }}
          >
            <Play size={14} />
            Start ({queuedCount})
          </button>
        ) : (
          <div className="flex items-center gap-2 px-4 py-2 text-sm" style={{ color: "var(--accent)" }}>
            <Loader2 size={14} className="animate-spin" />
            Processing...
          </div>
        )}
      </div>

      {/* Progress bar */}
      {(processing || doneCount > 0) && (
        <div className="h-1" style={{ background: "var(--bg-secondary)" }}>
          <div
            className="h-full transition-all duration-300"
            style={{ width: `${progressPct}%`, background: "var(--accent)" }}
          />
        </div>
      )}

      {/* Error banner */}
      {error && (
        <div
          className="mx-4 mt-3 p-3 rounded-lg text-sm flex items-start gap-2"
          style={{ background: "#fee2e2", color: "#991b1b" }}
        >
          <AlertCircle size={16} className="mt-0.5 shrink-0" />
          {error}
        </div>
      )}

      {/* Clip list */}
      <div className="flex-1 overflow-y-auto p-4">
        <div className="space-y-1">
          {clips.map((clip) => (
            <div key={clip.filename}>
              <div
                className="flex items-center gap-3 px-3 py-2 rounded-lg transition-colors"
                style={{ background: "var(--bg-card)" }}
              >
                <Film size={16} style={{ color: "var(--text-muted)" }} className="shrink-0" />
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-medium truncate">{clip.filename}</div>
                  <div className="text-xs" style={{ color: "var(--text-muted)" }}>
                    {clip.file_size ? formatSize(clip.file_size) : ""}
                    {clip.statusDetail && (
                      <span style={{ color: "var(--accent)" }}> · {clip.statusDetail}</span>
                    )}
                  </div>
                </div>
                {clip.status === "processing" && (
                  <Loader2 size={14} className="animate-spin" style={{ color: "var(--accent)" }} />
                )}
                {clip.status === "done" && (
                  <CheckCircle2 size={14} style={{ color: "var(--success)" }} />
                )}
                {clip.status === "error" && (
                  <AlertCircle size={14} style={{ color: "var(--error)" }} />
                )}
                {clip.status === "queued" && (
                  <Clock size={14} style={{ color: "var(--text-muted)" }} />
                )}
                <StatusPill status={clip.status} />
              </div>
              {/* Error detail */}
              {clip.status === "error" && clip.error && (
                <div
                  className="mx-3 mt-1 mb-1 px-3 py-2 rounded text-xs"
                  style={{ background: "#fee2e2", color: "#991b1b" }}
                >
                  {clip.error}
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
