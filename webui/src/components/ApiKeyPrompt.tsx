"use client";

import { useState } from "react";
import { setApiKey } from "@/lib/auth";
import { Button } from "@/components/ui/button";

interface ApiKeyPromptProps {
  onAuthenticated: () => void;
}

export function ApiKeyPrompt({ onAuthenticated }: ApiKeyPromptProps) {
  const [key, setKey] = useState("");
  const [error, setError] = useState(false);
  const [checking, setChecking] = useState(false);

  const submit = async () => {
    if (!key.trim()) return;
    setChecking(true);
    setError(false);
    try {
      const res = await fetch("/api/v1/system/status", {
        headers: { Authorization: `Bearer ${key.trim()}` },
      });
      if (res.ok) {
        setApiKey(key.trim());
        onAuthenticated();
      } else {
        setError(true);
      }
    } catch {
      setError(true);
    } finally {
      setChecking(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-background/80 backdrop-blur-sm" />
      <div className="relative z-10 w-full max-w-sm mx-4 rounded-2xl border border-border bg-card p-6 shadow-xl space-y-4">
        <div className="space-y-1">
          <h2 className="text-lg font-semibold">Authentication Required</h2>
          <p className="text-sm text-muted-foreground">
            This SnapDog instance requires an API key. Enter one of the keys
            configured in <code className="text-xs bg-muted px-1 py-0.5 rounded">api_keys</code> in
            your <code className="text-xs bg-muted px-1 py-0.5 rounded">snapdog.toml</code> file.
          </p>
        </div>
        <div className="space-y-2">
          <input
            type="password"
            placeholder="API key"
            value={key}
            onChange={(e: React.ChangeEvent<HTMLInputElement>) => { setKey(e.target.value); setError(false); }}
            onKeyDown={(e: React.KeyboardEvent) => e.key === "Enter" && submit()}
            autoFocus
            className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          />
          {error && (
            <p className="text-sm text-destructive">Invalid API key. Please try again.</p>
          )}
        </div>
        <Button onClick={submit} disabled={checking || !key.trim()} className="w-full">
          {checking ? "Checking…" : "Connect"}
        </Button>
        <p className="text-xs text-muted-foreground text-center">
          The key is stored in your browser&apos;s local storage and sent with every request.
        </p>
      </div>
    </div>
  );
}
