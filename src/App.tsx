import { useState } from "react";
import CodeEntry from "./components/CodeEntry";
import Scanner from "./components/Scanner";
import DeviceCard from "./components/DeviceCard";
import { invoke } from "@tauri-apps/api/core";

type AppState = "code-entry" | "scanning" | "results" | "complete";

interface SiteInfo {
  informUrl: string;
  siteId: string;
  siteName: string;
}

interface Device {
  mac: string;
  ip: string;
  model: string;
  firmware: string;
  hostname: string;
  isManaged: boolean;
}

interface ScanResult {
  devices: Device[];
}

interface AdoptResult {
  success: boolean;
  output: string;
}

export default function App() {
  const [state, setState] = useState<AppState>("code-entry");
  const [siteInfo, setSiteInfo] = useState<SiteInfo | null>(null);
  const [devices, setDevices] = useState<Device[]>([]);
  const [error, setError] = useState<string | null>(null);

  const handleCodeSubmit = async (code: string) => {
    setError(null);
    try {
      const result = await invoke<SiteInfo>("validate_code", { code });
      setSiteInfo(result);
      setState("scanning");
      handleScan();
    } catch (err) {
      setError(String(err));
    }
  };

  const handleScan = async () => {
    setState("scanning");
    setError(null);
    try {
      const result = await invoke<ScanResult>("scan_devices");
      setDevices(result.devices);
      setState("results");
    } catch (err) {
      setError(String(err));
      setState("results");
    }
  };

  const handleAdopt = async (ip: string) => {
    if (!siteInfo) return;
    setError(null);
    try {
      await invoke<AdoptResult>("adopt_device", {
        ip,
        informUrl: siteInfo.informUrl,
        customPassword: null,
      });
      setState("complete");
    } catch (err) {
      setError(String(err));
    }
  };

  const handleAdoptWithPassword = async (ip: string, password: string) => {
    if (!siteInfo) return;
    setError(null);
    try {
      await invoke<AdoptResult>("adopt_device", {
        ip,
        informUrl: siteInfo.informUrl,
        customPassword: password,
      });
      setState("complete");
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div className="min-h-screen bg-gray-50 flex flex-col">
      {/* Header */}
      <header className="bg-white border-b border-gray-200 px-6 py-4">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-vivaspot-primary rounded-lg flex items-center justify-center">
            <span className="text-white font-bold text-sm">VS</span>
          </div>
          <div>
            <h1 className="text-lg font-bold text-vivaspot-dark">
              VivaSpot Setup Assistant
            </h1>
            {siteInfo && (
              <p className="text-xs text-gray-500">
                Setting up: {siteInfo.siteName}
              </p>
            )}
          </div>
        </div>
      </header>

      {/* Main content */}
      <main className="flex-1 flex items-center justify-center p-6">
        <div className="w-full max-w-lg">
          {state === "code-entry" && (
            <CodeEntry onSubmit={handleCodeSubmit} error={error} />
          )}

          {state === "scanning" && <Scanner />}

          {state === "results" && (
            <div className="space-y-4">
              <div>
                <h2 className="text-xl font-bold text-vivaspot-dark">
                  {devices.length > 0
                    ? `Found ${devices.length} access point${devices.length > 1 ? "s" : ""}`
                    : "No access points found"}
                </h2>
                <p className="text-sm text-gray-600 mt-1">
                  {devices.length > 0
                    ? 'Click "Connect to VivaSpot" to set up your AP.'
                    : "We scanned your network but couldn't find any UniFi devices."}
                </p>
              </div>

              {/* Error display */}
              {error && (
                <div className="bg-red-50 border border-red-200 rounded-lg p-4 text-sm text-red-700">
                  {error}
                </div>
              )}

              {/* Device list */}
              {devices.length > 0 ? (
                <div className="space-y-3">
                  {devices.map((device) => (
                    <DeviceCard
                      key={device.mac}
                      device={device}
                      onAdopt={() => handleAdopt(device.ip)}
                      onAdoptWithPassword={(password) =>
                        handleAdoptWithPassword(device.ip, password)
                      }
                    />
                  ))}
                </div>
              ) : (
                <NoDevicesFound onRescan={handleScan} />
              )}

              {/* Rescan button */}
              {devices.length > 0 && (
                <button
                  onClick={handleScan}
                  className="w-full py-2 text-sm text-gray-600 hover:text-vivaspot-primary transition-colors"
                >
                  Scan again
                </button>
              )}
            </div>
          )}

          {state === "complete" && <SuccessScreen siteName={siteInfo?.siteName} />}
        </div>
      </main>
    </div>
  );
}

function NoDevicesFound({ onRescan }: { onRescan: () => void }) {
  return (
    <div className="bg-amber-50 border border-amber-200 rounded-lg p-5 space-y-3">
      <p className="text-sm text-amber-800 font-medium">
        This usually means one of three things:
      </p>
      <ol className="text-sm text-amber-700 space-y-2">
        <li className="flex items-start gap-2">
          <span className="font-bold">1.</span>
          <span>
            Is your AP powered on? The LED should be glowing white. If it's off,
            check the power cable and Ethernet connection.
          </span>
        </li>
        <li className="flex items-start gap-2">
          <span className="font-bold">2.</span>
          <span>
            Is your AP connected to the same router as this computer? The AP's
            Ethernet cable should plug into the same router or switch your
            computer is connected to.
          </span>
        </li>
        <li className="flex items-start gap-2">
          <span className="font-bold">3.</span>
          <span>
            Did you factory reset the AP? If the AP was previously set up with
            another system, it may not respond to scanning until it's reset.
            Hold the reset button for 10 seconds.
          </span>
        </li>
      </ol>
      <div className="flex gap-3 pt-2">
        <button
          onClick={onRescan}
          className="flex-1 py-2 px-4 bg-vivaspot-primary text-white rounded-lg text-sm font-medium hover:bg-vivaspot-primary-dark transition-colors"
        >
          Scan Again
        </button>
      </div>
    </div>
  );
}

function SuccessScreen({ siteName }: { siteName?: string }) {
  return (
    <div className="text-center space-y-4">
      <div className="w-16 h-16 bg-green-100 rounded-full flex items-center justify-center mx-auto">
        <svg
          className="w-8 h-8 text-green-600"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={2}
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M5 13l4 4L19 7"
          />
        </svg>
      </div>
      <h2 className="text-2xl font-bold text-vivaspot-dark">You're all set!</h2>
      <p className="text-gray-600">
        Your access point is now connected to VivaSpot
        {siteName ? ` for ${siteName}` : ""}. You can close this app and go back
        to the setup wizard in your browser — it will automatically detect your
        AP and finish the setup.
      </p>
      <button
        onClick={() => window.close()}
        className="mt-4 py-2 px-6 bg-gray-200 text-gray-700 rounded-lg text-sm font-medium hover:bg-gray-300 transition-colors"
      >
        Close
      </button>
    </div>
  );
}
