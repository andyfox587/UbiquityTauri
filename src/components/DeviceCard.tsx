import { useState } from "react";
import { Wifi, Lock, ChevronDown, ChevronUp } from "lucide-react";

interface Device {
  mac: string;
  ip: string;
  reportedIp: string;
  model: string;
  firmware: string;
  hostname: string;
  isManaged: boolean;
}

interface DeviceCardProps {
  device: Device;
  onAdopt: () => void;
  onAdoptWithPassword: (password: string) => void;
}

export default function DeviceCard({
  device,
  onAdopt,
  onAdoptWithPassword,
}: DeviceCardProps) {
  const [adopting, setAdopting] = useState(false);
  const [showPassword, setShowPassword] = useState(false);
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);

  const handleAdopt = async () => {
    setAdopting(true);
    setError(null);
    try {
      await onAdopt();
    } catch (err) {
      const errStr = String(err);
      if (errStr.toLowerCase().includes("authentication")) {
        setError("Your access point has a custom password. Factory reset it or enter the password below.");
        setShowPassword(true);
      } else {
        setError(errStr);
      }
    } finally {
      setAdopting(false);
    }
  };

  const handleAdoptWithPassword = async () => {
    if (!password.trim()) return;
    setAdopting(true);
    setError(null);
    try {
      await onAdoptWithPassword(password);
    } catch (err) {
      setError(String(err));
    } finally {
      setAdopting(false);
    }
  };

  if (device.isManaged) {
    return (
      <div className="border border-gray-200 rounded-lg p-4 bg-gray-50 opacity-60">
        <div className="flex items-start gap-3">
          <div className="w-10 h-10 bg-gray-200 rounded-lg flex items-center justify-center flex-shrink-0">
            <Lock className="w-5 h-5 text-gray-400" />
          </div>
          <div className="flex-1">
            <p className="text-sm font-medium text-gray-700">
              {device.model || "UniFi Device"}
            </p>
            <p className="text-xs text-gray-500">
              {device.mac} &middot; {device.ip}
            </p>
            <span className="inline-block mt-1 text-xs bg-gray-200 text-gray-600 px-2 py-0.5 rounded">
              Managed by another controller
            </span>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="border border-gray-200 rounded-lg p-4 bg-white">
      <div className="flex items-start gap-3">
        <div className="w-10 h-10 bg-vivaspot-light rounded-lg flex items-center justify-center flex-shrink-0">
          <Wifi className="w-5 h-5 text-vivaspot-primary" />
        </div>
        <div className="flex-1">
          <p className="text-sm font-medium text-vivaspot-dark">
            {device.model || "UniFi Access Point"}
          </p>
          <p className="text-xs text-gray-500">
            {device.mac} &middot; {device.ip}
          </p>
          {device.firmware && (
            <p className="text-xs text-gray-400 mt-0.5">
              Firmware: {device.firmware}
            </p>
          )}
        </div>
      </div>

      {/* Error */}
      {error && (
        <div className="mt-3 bg-amber-50 border border-amber-200 rounded-lg p-3 text-xs text-amber-700">
          {error}
        </div>
      )}

      {/* Password entry (shown when auth fails) */}
      {showPassword && (
        <div className="mt-3 space-y-2">
          <button
            onClick={() => setShowPassword(!showPassword)}
            className="text-xs text-vivaspot-primary flex items-center gap-1"
          >
            I know the password
            {showPassword ? (
              <ChevronUp className="w-3 h-3" />
            ) : (
              <ChevronDown className="w-3 h-3" />
            )}
          </button>
          <div className="flex gap-2">
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="SSH password"
              className="flex-1 text-sm border border-gray-300 rounded-lg px-3 py-1.5 focus:border-vivaspot-primary focus:outline-none"
            />
            <button
              onClick={handleAdoptWithPassword}
              disabled={!password.trim() || adopting}
              className="px-4 py-1.5 bg-vivaspot-primary text-white text-sm rounded-lg hover:bg-vivaspot-primary-dark disabled:bg-gray-200 disabled:text-gray-400 transition-colors"
            >
              Connect
            </button>
          </div>
        </div>
      )}

      {/* Adopt button */}
      <button
        onClick={handleAdopt}
        disabled={adopting}
        className={`mt-3 w-full py-2 px-4 rounded-lg text-sm font-medium transition-colors ${
          adopting
            ? "bg-gray-100 text-gray-400"
            : "bg-vivaspot-primary text-white hover:bg-vivaspot-primary-dark"
        }`}
      >
        {adopting ? (
          <span className="flex items-center justify-center gap-2">
            <svg
              className="animate-spin w-4 h-4"
              viewBox="0 0 24 24"
              fill="none"
            >
              <circle
                className="opacity-25"
                cx="12"
                cy="12"
                r="10"
                stroke="currentColor"
                strokeWidth="4"
              />
              <path
                className="opacity-75"
                fill="currentColor"
                d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
              />
            </svg>
            Connecting...
          </span>
        ) : (
          "Connect to VivaSpot"
        )}
      </button>
    </div>
  );
}
