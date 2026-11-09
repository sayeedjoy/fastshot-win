import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AppWindow, LogOut, Monitor, ShieldCheck, SquareDashed, Timer, TriangleAlert, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";

import { Alert, AlertAction, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Kbd, KbdGroup } from "@/components/ui/kbd";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { Toaster } from "@/components/ui/sonner";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { TooltipProvider } from "@/components/ui/tooltip";
import { type AppInfo, type CaptureMode, type MainView, type NavigatePayload, type Settings, api, errorMessage } from "@/shared/ipc";
import { isMac, setThemePreference } from "@/shared/theme";

import { formatAccelerator } from "./accelerator";
import { Onboarding } from "./Onboarding";
import { PermissionPanel } from "./PermissionPanel";
import { AnnotationSettings, GeneralSettings } from "./SettingsForm";

export default function SettingsApp() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [view, setView] = useState<MainView>("settings");
  const [banner, setBanner] = useState<string | null>(null);
  const [shortcutError, setShortcutError] = useState<string | null>(null);
  const [delay, setDelay] = useState("0");

  const refresh = useCallback(async () => {
    const [s, i] = await Promise.all([api.getSettings(), api.getAppInfo()]);
    setSettings(s);
    setInfo(i);
    setShortcutError(i.shortcutError);
    setThemePreference(s.theme);
    if (!s.onboardingComplete) setView("onboarding");
  }, []);

  useEffect(() => {
    void refresh();
    const un = listen<NavigatePayload>("navigate", (e) => {
      setView(e.payload.view);
      setBanner(e.payload.error);
      void refresh();
    });
    return () => {
      void un.then((f) => f());
    };
  }, [refresh]);

  const save = useCallback(
    async (patch: Partial<Settings>) => {
      if (!settings) return;
      const next = { ...settings, ...patch };
      try {
        const saved = await api.updateSettings(next);
        setSettings(saved);
        setThemePreference(saved.theme);
        if ("shortcut" in patch) {
          setShortcutError(null);
          toast.success("Shortcut updated");
        }
      } catch (e) {
        const msg = errorMessage(e);
        if ("shortcut" in patch) setShortcutError(msg);
        else toast.error(msg);
      }
    },
    [settings],
  );

  const capture = (mode: CaptureMode, delaySecs = Number(delay)) => void api.beginCapture(mode, delaySecs);

  if (!settings) return null;

  if (view === "onboarding") {
    return (
      <>
        <Onboarding
          settings={settings}
          info={info}
          onFinish={async (captureNow) => {
            await save({ onboardingComplete: true });
            setView("settings");
            if (captureNow) capture("region", 0);
          }}
        />
        <Toaster />
      </>
    );
  }

  return (
    <TooltipProvider>
      <div className="flex min-h-svh flex-col">
        <header className="flex flex-wrap items-center justify-between gap-3 border-b px-6 py-4">
          <div className="flex flex-col gap-0.5">
            <h1 className="font-heading text-lg font-semibold">FastShot</h1>
            <p className="flex items-center gap-1.5 text-sm text-muted-foreground">
              Press
              <KbdGroup>
                {formatAccelerator(settings.shortcut, isMac).map((k) => (
                  <Kbd key={k}>{k}</Kbd>
                ))}
              </KbdGroup>
              to capture
            </p>
          </div>
          <div className="flex flex-wrap gap-2">
            <Select value={delay} onValueChange={setDelay}>
              <SelectTrigger aria-label="Capture delay">
                <Timer />
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="0">No delay</SelectItem>
                <SelectItem value="3">3 s delay</SelectItem>
                <SelectItem value="5">5 s delay</SelectItem>
                <SelectItem value="10">10 s delay</SelectItem>
              </SelectContent>
            </Select>
            <Button onClick={() => capture("region")}>
              <SquareDashed data-icon="inline-start" />
              Region
            </Button>
            <Button variant="outline" onClick={() => capture("window")}>
              <AppWindow data-icon="inline-start" />
              Window
            </Button>
            <Button variant="outline" onClick={() => capture("display")}>
              <Monitor data-icon="inline-start" />
              Display
            </Button>
          </div>
        </header>

        <main className="flex flex-1 flex-col gap-4 px-6 py-5">
          {banner ? (
            <Alert variant="destructive">
              <TriangleAlert />
              <AlertTitle>Something needs attention</AlertTitle>
              <AlertDescription>{banner}</AlertDescription>
              <AlertAction>
                <Button variant="ghost" size="icon-sm" aria-label="Dismiss" onClick={() => setBanner(null)}>
                  <X />
                </Button>
              </AlertAction>
            </Alert>
          ) : null}

          {view === "permission" ? (
            <section className="flex flex-col gap-3" aria-labelledby="perm-title">
              <h2 id="perm-title" className="font-heading text-base font-semibold">
                Screen recording permission needed
              </h2>
              <PermissionPanel />
              <div>
                <Button variant="ghost" onClick={() => setView("settings")}>
                  Back to settings
                </Button>
              </div>
            </section>
          ) : (
            <Tabs defaultValue="general">
              <TabsList>
                <TabsTrigger value="general">General</TabsTrigger>
                <TabsTrigger value="annotations">Annotations</TabsTrigger>
                <TabsTrigger value="privacy">Privacy &amp; about</TabsTrigger>
              </TabsList>
              <TabsContent value="general" className="pt-4">
                <GeneralSettings settings={settings} save={save} shortcutError={shortcutError} />
              </TabsContent>
              <TabsContent value="annotations" className="pt-4">
                <AnnotationSettings settings={settings} save={save} shortcutError={null} />
              </TabsContent>
              <TabsContent value="privacy" className="flex flex-col gap-4 pt-4">
                <Alert>
                  <ShieldCheck />
                  <AlertTitle>Redact is permanent. Blur and pixelate are not.</AlertTitle>
                  <AlertDescription>
                    Redact replaces pixels with solid colour in the exported image. Blur and pixelate are visual effects — text beneath them may be recoverable.
                  </AlertDescription>
                </Alert>
                <ul className="flex list-disc flex-col gap-1.5 pl-5 text-sm text-muted-foreground">
                  <li>Works fully offline. No account, no uploads, no analytics.</li>
                  <li>Screenshots stay in memory until you copy or save; nothing is written to disk automatically.</li>
                  <li>Clipboard managers and OS clipboard history may keep copies of anything you copy.</li>
                  <li>Exported images are freshly encoded without metadata.</li>
                </ul>
                <Separator />
                <div className="flex flex-wrap items-center justify-between gap-2 text-sm text-muted-foreground">
                  <span>
                    Version {info?.version} · {info?.platform}
                  </span>
                  <div className="flex gap-2">
                    <Button variant="outline" onClick={() => setView("onboarding")}>
                      Show welcome
                    </Button>
                    <Button variant="outline" onClick={() => void api.quitApp()}>
                      <LogOut data-icon="inline-start" />
                      Quit FastShot
                    </Button>
                  </div>
                </div>
              </TabsContent>
            </Tabs>
          )}
        </main>
        <footer className="flex justify-end border-t px-6 py-3">
          <Button variant="ghost" onClick={() => void getCurrentWindow().hide()}>
            Close
          </Button>
        </footer>
      </div>
      <Toaster />
    </TooltipProvider>
  );
}
