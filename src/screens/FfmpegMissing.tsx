import { useState } from "react";
import { ipc } from "../ipc";
import { AlertTriangle, RefreshCw, ExternalLink, Terminal } from "lucide-react";

interface Props {
  onResolved: () => void;
}

export function FfmpegMissingScreen({ onResolved }: Props) {
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const platform = navigator.userAgent.includes("Mac") ? "mac" : "windows";

  async function retry() {
    setChecking(true);
    setError(null);
    try {
      await ipc.checkFfmpeg();
      onResolved();
    } catch (e) {
      setError(String(e));
    } finally {
      setChecking(false);
    }
  }

  return (
    <div className="flex-1 flex flex-col items-center justify-center p-8">
      <div className="w-full max-w-lg text-center">
        <AlertTriangle
          size={56}
          className="mx-auto mb-6"
          style={{ color: "var(--warning)" }}
        />

        <h1 className="text-2xl font-bold mb-2">FFmpeg Not Found</h1>
        <p className="text-sm mb-8" style={{ color: "var(--text-muted)" }}>
          Rushlog requires FFmpeg to process video files. Install it and click
          "Check Again" below.
        </p>

        <div
          className="rounded-xl p-6 text-left mb-6"
          style={{
            background: "var(--bg-card)",
            border: "1px solid var(--border)",
          }}
        >
          {platform === "mac" ? (
            <>
              <h2 className="font-semibold mb-3 flex items-center gap-2">
                <Terminal size={16} />
                macOS
              </h2>
              <p className="text-sm mb-2" style={{ color: "var(--text-muted)" }}>
                Install with Homebrew (recommended):
              </p>
              <code
                className="block rounded-lg px-4 py-3 text-sm font-mono mb-4 select-all"
                style={{ background: "var(--bg-secondary)" }}
              >
                brew install ffmpeg
              </code>
              <p className="text-sm" style={{ color: "var(--text-muted)" }}>
                Don't have Homebrew?{" "}
                <a
                  href="https://brew.sh"
                  target="_blank"
                  rel="noreferrer"
                  className="underline inline-flex items-center gap-1"
                  style={{ color: "var(--accent)" }}
                >
                  Install it first
                  <ExternalLink size={12} />
                </a>
              </p>
            </>
          ) : (
            <>
              <h2 className="font-semibold mb-3 flex items-center gap-2">
                <Terminal size={16} />
                Windows
              </h2>
              <p className="text-sm mb-2" style={{ color: "var(--text-muted)" }}>
                Option 1 — Install with winget:
              </p>
              <code
                className="block rounded-lg px-4 py-3 text-sm font-mono mb-4 select-all"
                style={{ background: "var(--bg-secondary)" }}
              >
                winget install Gyan.FFmpeg
              </code>
              <p className="text-sm mb-2" style={{ color: "var(--text-muted)" }}>
                Option 2 — Download manually:
              </p>
              <a
                href="https://ffmpeg.org/download.html"
                target="_blank"
                rel="noreferrer"
                className="text-sm underline inline-flex items-center gap-1"
                style={{ color: "var(--accent)" }}
              >
                ffmpeg.org/download.html
                <ExternalLink size={12} />
              </a>
              <p
                className="text-sm mt-2"
                style={{ color: "var(--text-muted)" }}
              >
                After downloading, add the <code>bin</code> folder to your
                system PATH.
              </p>
            </>
          )}
        </div>

        <button
          onClick={retry}
          disabled={checking}
          className="inline-flex items-center gap-2 px-6 py-3 rounded-lg font-medium transition-opacity hover:opacity-90 disabled:opacity-50"
          style={{ background: "var(--accent)", color: "#fff" }}
        >
          <RefreshCw size={16} className={checking ? "animate-spin" : ""} />
          {checking ? "Checking..." : "Check Again"}
        </button>

        {error && (
          <div
            className="mt-4 p-3 rounded-lg text-sm"
            style={{ background: "var(--bg-secondary)", color: "var(--error)" }}
          >
            {error}
          </div>
        )}
      </div>
    </div>
  );
}
