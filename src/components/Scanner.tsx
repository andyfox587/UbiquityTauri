import { Wifi } from "lucide-react";

export default function Scanner() {
  return (
    <div className="text-center space-y-6">
      {/* Animated scanning icon */}
      <div className="relative w-24 h-24 mx-auto">
        <div className="absolute inset-0 rounded-full bg-vivaspot-primary/10 animate-ping" />
        <div className="absolute inset-2 rounded-full bg-vivaspot-primary/20 animate-ping animation-delay-200" />
        <div className="relative w-24 h-24 rounded-full bg-vivaspot-light flex items-center justify-center">
          <Wifi className="w-10 h-10 text-vivaspot-primary" />
        </div>
      </div>

      <div>
        <h2 className="text-xl font-bold text-vivaspot-dark">
          Scanning your network...
        </h2>
        <p className="text-sm text-gray-600 mt-2">
          Looking for UniFi access points on your local network.
          <br />
          This usually takes 3-5 seconds.
        </p>
      </div>

      {/* Progress dots */}
      <div className="flex justify-center gap-1.5">
        <div className="w-2 h-2 bg-vivaspot-primary rounded-full animate-bounce" />
        <div
          className="w-2 h-2 bg-vivaspot-primary rounded-full animate-bounce"
          style={{ animationDelay: "0.1s" }}
        />
        <div
          className="w-2 h-2 bg-vivaspot-primary rounded-full animate-bounce"
          style={{ animationDelay: "0.2s" }}
        />
      </div>
    </div>
  );
}
