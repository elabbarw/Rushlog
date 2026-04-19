import { useState } from "react";
import "./App.css";
import { ScanResult } from "./ipc";
import { PickerScreen } from "./screens/Picker";
import { QueueScreen } from "./screens/Queue";
import { DoneScreen } from "./screens/Done";
import { SettingsScreen } from "./screens/Settings";

type Screen = "picker" | "queue" | "done" | "settings";

function App() {
  const [screen, setScreen] = useState<Screen>("picker");
  const [scanResult, setScanResult] = useState<ScanResult | null>(null);

  return (
    <div className="flex flex-col min-h-screen" style={{ background: "var(--bg)", color: "var(--text)" }}>
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
