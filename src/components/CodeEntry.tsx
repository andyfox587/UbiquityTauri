import { useState } from "react";
import { KeyRound } from "lucide-react";

interface CodeEntryProps {
  onSubmit: (code: string) => void;
  error: string | null;
}

export default function CodeEntry({ onSubmit, error }: CodeEntryProps) {
  const [code, setCode] = useState("");
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!code.trim()) return;

    setLoading(true);
    try {
      await onSubmit(code.trim().toUpperCase());
    } finally {
      setLoading(false);
    }
  };

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    // Auto-format: uppercase, allow VS- prefix
    let value = e.target.value.toUpperCase();
    // Auto-add VS- prefix if user types just the code part
    if (value.length === 4 && !value.startsWith("VS-")) {
      value = "VS-" + value;
    }
    setCode(value);
  };

  return (
    <div className="text-center space-y-6">
      <div className="w-16 h-16 bg-vivaspot-light rounded-full flex items-center justify-center mx-auto">
        <KeyRound className="w-8 h-8 text-vivaspot-primary" />
      </div>

      <div>
        <h2 className="text-2xl font-bold text-vivaspot-dark">
          Enter your setup code
        </h2>
        <p className="text-sm text-gray-600 mt-2">
          You'll find this code in the VivaSpot setup wizard in your browser.
          <br />
          It looks like <span className="font-mono font-bold">VS-7K2M</span>.
        </p>
      </div>

      <form onSubmit={handleSubmit} className="space-y-4">
        <input
          type="text"
          value={code}
          onChange={handleChange}
          placeholder="VS-XXXX"
          className="w-full text-center text-3xl font-mono font-bold tracking-widest py-4 px-6 border-2 border-gray-300 rounded-xl focus:border-vivaspot-primary focus:outline-none transition-colors bg-white"
          maxLength={7}
          autoFocus
          disabled={loading}
        />

        {error && (
          <div className="bg-red-50 border border-red-200 rounded-lg p-3 text-sm text-red-700">
            {error}
          </div>
        )}

        <button
          type="submit"
          disabled={!code.trim() || loading}
          className={`w-full py-3 px-6 rounded-xl text-sm font-medium transition-colors ${
            code.trim() && !loading
              ? "bg-vivaspot-primary text-white hover:bg-vivaspot-primary-dark"
              : "bg-gray-200 text-gray-400 cursor-not-allowed"
          }`}
        >
          {loading ? (
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
              Verifying...
            </span>
          ) : (
            "Continue"
          )}
        </button>
      </form>
    </div>
  );
}
