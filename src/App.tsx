import { useEffect, useState } from "react";
import "./App.css";
import { ipc, ScanResult } from "./ipc";
import { FfmpegMissingScreen } from "./screens/FfmpegMissing";
import { PickerScreen } from "./screens/Picker";
import { QueueScreen } from "./screens/Queue";
import { DoneScreen } from "./screens/Done";
import { SettingsScreen } from "./screens/Settings";

type Screen = "loading" | "ffmpeg-missing" | "picker" | "queue" | "done" | "settings";

function App() {
  const [screen, setScreen] = useState<Screen>("loading");
  const [scanResult, setScanResult] = useState<ScanResult | null>(null);

  useEffect(() => {
    ipc.checkFfmpeg()
      .then(() => setScreen("picker"))
      .catch(() => setScreen("ffmpeg-missing"));
  }, []);

  return (
    <div className="flex flex-col min-h-screen" style={{ background: "var(--bg)", color: "var(--text)" }}>
      {screen === "loading" && (
        <div className="flex-1 flex items-center justify-center">
          <p style={{ color: "var(--text-muted)" }}>Starting up...</p>
        </div>
      )}
      {screen === "ffmpeg-missing" && (
        <FfmpegMissingScreen onResolved={() => setScreen("picker")} />
      )}
      {screen === "picker" && (
        <PickerScreen
          onFolderScanned={(result) => {
            setScanResult(result);
            setScreen("queue");
          }}
          onOpenSettings={() => setScreen("settings")}
        />
      )}
      {screen === "queue" && scanResult && (
        <QueueScreen
          scanResult={scanResult}
          onComplete={() => setScreen("done")}
          onBack={() => setScreen("picker")}
        />
      )}
      {screen === "done" && scanResult && (
        <DoneScreen
          folderPath={scanResult.folder_path}
          onNewFolder={() => {
            setScanResult(null);
            setScreen("picker");
          }}
        />
      )}
      {screen === "settings" && (
        <SettingsScreen onBack={() => setScreen("picker")} />
      )}
    </div>
  );
}

export default App;
