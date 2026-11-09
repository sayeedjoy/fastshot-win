import { Droplets, MonitorUp, ShieldCheck } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Kbd, KbdGroup } from "@/components/ui/kbd";
import type { AppInfo, Settings } from "@/shared/ipc";
import { isMac } from "@/shared/theme";

import { formatAccelerator } from "./accelerator";
import { PermissionPanel } from "./PermissionPanel";

interface Props {
  settings: Settings;
  info: AppInfo | null;
  onFinish: (captureNow: boolean) => void;
}

/** Lightweight, skippable first run (SYS-03). */
export function Onboarding({ settings, info, onFinish }: Props) {
  const needsPermission = info?.platform === "macos" && info.permission === "denied";
  const steps = ["shortcut", "redact", "tray", ...(needsPermission ? ["permission"] : [])] as const;
  const [i, setI] = useState(0);
  const step = steps[i];
  const last = i === steps.length - 1;

  return (
    <div className="flex min-h-svh items-center justify-center p-6">
      <Card className="w-full max-w-lg">
        {step === "shortcut" ? (
          <CardHeader>
            <img src="/favicon.svg" alt="" aria-hidden className="mb-2 size-12" draggable={false} />
            <CardTitle className="text-xl">Welcome to FastShot</CardTitle>
            <CardDescription>Take a screenshot, mark it up, hide what's private, and share it. Nothing leaves your device.</CardDescription>
          </CardHeader>
        ) : step === "redact" ? (
          <CardHeader>
            <ShieldCheck className="mb-2 text-primary" aria-hidden />
            <CardTitle className="text-xl">Redact vs. blur</CardTitle>
            <CardDescription>Know which one protects secrets.</CardDescription>
          </CardHeader>
        ) : step === "tray" ? (
          <CardHeader>
            <MonitorUp className="mb-2 text-primary" aria-hidden />
            <CardTitle className="text-xl">FastShot lives in your {isMac ? "menu bar" : "system tray"}</CardTitle>
            <CardDescription>Closing this window keeps it running so the shortcut always works.</CardDescription>
          </CardHeader>
        ) : (
          <CardHeader>
            <CardTitle className="text-xl">Allow screen recording</CardTitle>
            <CardDescription>macOS asks once before an app can capture the screen.</CardDescription>
          </CardHeader>
        )}

        <CardContent>
          {step === "shortcut" ? (
            <div className="flex flex-col gap-3 text-sm">
              <p className="flex flex-wrap items-center gap-2">
                Press
                <KbdGroup>
                  {formatAccelerator(settings.shortcut, isMac).map((k) => (
                    <Kbd key={k}>{k}</Kbd>
                  ))}
                </KbdGroup>
                anywhere to capture.
              </p>
              <p className="text-muted-foreground">Drag to select an area, then click Copy. You can change the shortcut in Settings.</p>
            </div>
          ) : step === "redact" ? (
            <div className="flex flex-col gap-4 text-sm">
              <div className="flex gap-3">
                <ShieldCheck className="mt-0.5 shrink-0 text-primary" aria-hidden />
                <p>
                  <span className="font-medium">Redact</span> permanently replaces pixels with a solid colour in the image you copy or save. The hidden content can't be recovered from that image.
                </p>
              </div>
              <div className="flex gap-3">
                <Droplets className="mt-0.5 shrink-0 text-muted-foreground" aria-hidden />
                <p>
                  <span className="font-medium">Blur</span> and <span className="font-medium">Pixelate</span> are visual effects. Text under them can sometimes be reconstructed, so don't use them for passwords or personal data.
                </p>
              </div>
            </div>
          ) : step === "tray" ? (
            <p className="text-sm text-muted-foreground">
              {isMac ? "Click the menu-bar icon" : "Click the tray icon to capture a region, or right-click it"} for Capture Region, Window, Display, Settings and Quit.
            </p>
          ) : (
            <PermissionPanel compact />
          )}
        </CardContent>

        <CardFooter className="flex justify-between gap-2">
          <Button variant="ghost" onClick={() => onFinish(false)}>
            Skip
          </Button>
          <div className="flex gap-2">
            {i > 0 ? (
              <Button variant="outline" onClick={() => setI(i - 1)}>
                Back
              </Button>
            ) : null}
            {last ? (
              <Button onClick={() => onFinish(true)}>Take a screenshot</Button>
            ) : (
              <Button onClick={() => setI(i + 1)}>Next</Button>
            )}
          </div>
        </CardFooter>
        <p className="px-6 pb-4 text-xs text-muted-foreground" aria-live="polite">
          Step {i + 1} of {steps.length}
        </p>
      </Card>
    </div>
  );
}
