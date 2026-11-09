import { ExternalLink, RefreshCw } from "lucide-react";
import { useState } from "react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { api, errorMessage } from "@/shared/ipc";

/**
 * macOS screen-recording permission recovery (PRD Flow C). Never loops on
 * prompts: the system prompt is requested at most once; after that we only
 * deep-link to System Settings.
 */
export function PermissionPanel({ compact = false }: { compact?: boolean }) {
  const [status, setStatus] = useState<"idle" | "granted" | "denied">("idle");
  const [error, setError] = useState<string | null>(null);

  const check = async () => {
    try {
      const info = await api.getAppInfo();
      setStatus(info.permission);
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  return (
    <div className="flex flex-col gap-4">
      {!compact ? (
        <p className="text-sm text-muted-foreground">
          FastShot needs <span className="font-medium text-foreground">Screen &amp; System Audio Recording</span> access to capture your screen. It only reads the screen when you press the shortcut,
          and nothing is uploaded.
        </p>
      ) : null}
      <ol className="flex list-decimal flex-col gap-1 pl-5 text-sm">
        <li>Open System Settings → Privacy &amp; Security → Screen &amp; System Audio Recording.</li>
        <li>Turn on FastShot.</li>
        <li>If macOS asks, quit and reopen FastShot.</li>
      </ol>
      <div className="flex flex-wrap gap-2">
        <Button
          onClick={async () => {
            try {
              // First call shows the system prompt; later calls return at once.
              const r = await api.requestPermission();
              setStatus(r);
              if (r !== "granted") await api.openPermissionSettings();
            } catch (e) {
              setError(errorMessage(e));
            }
          }}
        >
          <ExternalLink data-icon="inline-start" />
          Open System Settings
        </Button>
        <Button variant="outline" onClick={check}>
          <RefreshCw data-icon="inline-start" />
          Check again
        </Button>
      </div>
      {status === "granted" ? (
        <Alert>
          <AlertTitle>Access granted</AlertTitle>
          <AlertDescription>You're ready to capture.</AlertDescription>
        </Alert>
      ) : status === "denied" ? (
        <Alert>
          <AlertTitle>Not granted yet</AlertTitle>
          <AlertDescription>After turning FastShot on, macOS may need you to quit and reopen the app before it takes effect.</AlertDescription>
        </Alert>
      ) : null}
      {error ? (
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}
    </div>
  );
}
