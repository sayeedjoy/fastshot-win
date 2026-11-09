import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AppWindow, Brush, Camera, Info, LogOut, Monitor, Settings2, ShieldCheck, Sparkles, SquareDashed, Timer, TriangleAlert, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";

import { Alert, AlertAction, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Kbd, KbdGroup } from "@/components/ui/kbd";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Toaster } from "@/components/ui/sonner";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { TooltipProvider } from "@/components/ui/tooltip";
import { type AppInfo, type CaptureMode, type MainView, type NavigatePayload, type Settings, api, errorMessage } from "@/shared/ipc";
import { isMac, setThemePreference } from "@/shared/theme";

import { formatAccelerator } from "./accelerator";
import { Onboarding } from "./Onboarding";
import { PermissionPanel } from "./PermissionPanel";
import { AnnotationSettings, GeneralSettings, SettingsSection } from "./SettingsForm";

const TABS = [
  { value: "general", label: "General", icon: Settings2, hint: "Capture, saving and startup" },
  { value: "annotations", label: "Annotations", icon: Brush, hint: "Default colours and sizes for the editor" },
  { value: "privacy", label: "Privacy & about", icon: ShieldCheck, hint: "How FastShot handles your screenshots" },
] as const;

const PRIVACY_POINTS = [
  "Works fully offline. No account, no uploads, no analytics.",
  "Screenshots stay in memory until you copy or save; nothing is written to disk automatically.",
  "Clipboard managers and OS clipboard history may keep copies of anything you copy.",
  "Exported images are freshly encoded without metadata.",
];

export default function SettingsApp() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [view, setView] = useState<MainView>("settings");
  const [banner, setBanner] = useState<string | null>(null);
  const [shortcutError, setShortcutError] = useState<string | null>(null);
  const [delay, setDelay] = useState("0");
  const [tab, setTab] = useState<string>("general");

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

  const current = TABS.find((t) => t.value === tab) ?? TABS[0];

  return (
    <TooltipProvider>
      <div className="flex h-svh flex-col overflow-hidden bg-background">
        <header className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b px-5 py-3">
          <div className="flex items-center gap-3">
            <span className="flex size-9 items-center justify-center rounded-lg bg-primary text-primary-foreground shadow-sm">
              <Camera className="size-5" />
            </span>
            <div className="flex flex-col gap-1">
              <h1 className="font-heading text-base leading-none font-semibold">FastShot</h1>
              <p className="flex items-center gap-1.5 text-xs text-muted-foreground">
                Press
                <KbdGroup>
                  {formatAccelerator(settings.shortcut, isMac).map((k) => (
                    <Kbd key={k}>{k}</Kbd>
                  ))}
                </KbdGroup>
                to capture
              </p>
            </div>
          </div>
          <div className="flex flex-wrap items-center gap-2">
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

        {banner ? (
          <div className="shrink-0 px-5 pt-4">
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
          </div>
        ) : null}

        {view === "permission" ? (
          <main className="flex-1 overflow-y-auto px-6 py-6">
            <section className="mx-auto flex max-w-xl flex-col gap-3" aria-labelledby="perm-title">
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
          </main>
        ) : (
          <Tabs value={tab} onValueChange={setTab} orientation="vertical" className="min-h-0 flex-1 gap-0">
            <aside className="flex w-48 shrink-0 flex-col justify-between border-r bg-muted/30 p-3">
              <TabsList className="h-fit w-full flex-col items-stretch gap-1 bg-transparent p-0">
                {TABS.map(({ value, label, icon: Icon }) => (
                  <TabsTrigger
                    key={value}
                    value={value}
                    className="h-9 flex-none justify-start gap-2.5 rounded-md px-2.5 text-[0.8rem] data-active:bg-background data-active:shadow-sm dark:data-active:bg-input/40 [&_svg]:text-muted-foreground data-active:[&_svg]:text-primary"
                  >
                    <Icon />
                    {label}
                  </TabsTrigger>
                ))}
              </TabsList>
              <div className="flex flex-col gap-1">
                <Button variant="ghost" size="sm" className="justify-start text-muted-foreground" onClick={() => void getCurrentWindow().hide()}>
                  <X data-icon="inline-start" />
                  Close window
                </Button>
                {info ? <p className="px-2.5 text-[11px] text-muted-foreground/80">Version {info.version}</p> : null}
              </div>
            </aside>

            <main className="min-w-0 flex-1 overflow-y-auto">
              <div className="mx-auto flex max-w-2xl flex-col gap-6 px-6 py-6">
                <div className="flex flex-col gap-1">
                  <h2 className="flex items-center gap-2 font-heading text-xl font-semibold tracking-tight">
                    <current.icon className="size-5 text-primary" />
                    {current.label}
                  </h2>
                  <p className="text-sm text-muted-foreground">{current.hint}. Changes are saved automatically.</p>
                </div>

                <TabsContent value="general">
                  <GeneralSettings settings={settings} save={save} shortcutError={shortcutError} defaultSaveDirectory={info?.defaultSaveDirectory} />
                </TabsContent>
                <TabsContent value="annotations">
                  <AnnotationSettings settings={settings} save={save} shortcutError={null} />
                </TabsContent>
                <TabsContent value="privacy" className="flex flex-col gap-8">
                  <Alert>
                    <ShieldCheck />
                    <AlertTitle>Redact is permanent. Blur and pixelate are not.</AlertTitle>
                    <AlertDescription>
                      Redact replaces pixels with solid colour in the exported image. Blur and pixelate are visual effects — text beneath them may be recoverable.
                    </AlertDescription>
                  </Alert>
                  <SettingsSection icon={ShieldCheck} title="Your data">
                    <ul className="flex flex-col gap-2.5 text-sm text-muted-foreground">
                      {PRIVACY_POINTS.map((t) => (
                        <li key={t} className="flex gap-2.5">
                          <span className="mt-1.5 size-1.5 shrink-0 rounded-full bg-primary/60" />
                          {t}
                        </li>
                      ))}
                    </ul>
                  </SettingsSection>
                  <SettingsSection icon={Info} title="About">
                    <div className="flex flex-wrap items-center justify-between gap-3">
                      <div className="flex flex-col gap-0.5 text-sm">
                        <span className="font-medium">FastShot {info?.version}</span>
                        <span className="text-xs text-muted-foreground">Running on {info?.platform}</span>
                      </div>
                      <div className="flex flex-wrap gap-2">
                        <Button variant="outline" size="sm" onClick={() => setView("onboarding")}>
                          <Sparkles data-icon="inline-start" />
                          Show welcome
                        </Button>
                        <Button variant="outline" size="sm" className="text-destructive hover:text-destructive" onClick={() => void api.quitApp()}>
                          <LogOut data-icon="inline-start" />
                          Quit FastShot
                        </Button>
                      </div>
                    </div>
                  </SettingsSection>
                </TabsContent>
              </div>
            </main>
          </Tabs>
        )}
      </div>
      <Toaster />
    </TooltipProvider>
  );
}
