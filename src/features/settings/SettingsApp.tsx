import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AppWindow, Brush, Info, LogOut, Monitor, Settings2, ShieldCheck, Sparkles, SquareDashed, TriangleAlert, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";

import { Alert, AlertAction, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Kbd, KbdGroup } from "@/components/ui/kbd";
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

function FastShotMark() {
  return (
    <img src="/favicon.svg" alt="" aria-hidden className="block size-8 shrink-0" draggable={false} />
  );
}

function SettingsSkeleton() {
  return (
    <div className="flex h-svh flex-col overflow-hidden bg-frame" aria-label="Loading settings" aria-busy="true">
      <header className="flex h-16 shrink-0 items-center px-5">
        <div className="flex items-center gap-3">
          <FastShotMark />
          <div className="h-4 w-20 rounded bg-foreground/10" />
        </div>
      </header>
      <div className="flex min-h-0 flex-1 gap-2 px-2 pb-2">
        <div className="hidden w-44 shrink-0 rounded-xl bg-foreground/5 min-[681px]:block" />
        <div className="flex-1 rounded-2xl border border-sheet-edge bg-background p-7">
          <div className="h-6 w-32 rounded bg-muted" />
          <div className="mt-3 h-4 w-64 max-w-full rounded bg-muted" />
          <div className="mt-8 h-44 rounded-xl border bg-muted/30" />
        </div>
      </div>
    </div>
  );
}

function useCompactSettingsLayout() {
  const [compact, setCompact] = useState(() => window.matchMedia("(max-width: 680px)").matches);

  useEffect(() => {
    const query = window.matchMedia("(max-width: 680px)");
    const update = (event: MediaQueryListEvent) => setCompact(event.matches);
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, []);

  return compact;
}

export default function SettingsApp() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [view, setView] = useState<MainView>(() => window.__FASTSHOT_MAIN__?.view ?? "settings");
  const [banner, setBanner] = useState<string | null>(() => window.__FASTSHOT_MAIN__?.error ?? null);
  const [shortcutError, setShortcutError] = useState<string | null>(null);
  const [tab, setTab] = useState<string>("general");
  const compactLayout = useCompactSettingsLayout();

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

  const capture = (mode: CaptureMode) => void api.beginCapture(mode);

  if (!settings) return <SettingsSkeleton />;

  if (view === "onboarding") {
    return (
      <>
        <Onboarding
          settings={settings}
          info={info}
          onFinish={async (captureNow) => {
            await save({ onboardingComplete: true });
            setView("settings");
            if (captureNow) capture("region");
          }}
        />
        <Toaster />
      </>
    );
  }

  const current = TABS.find((t) => t.value === tab) ?? TABS[0];

  return (
    <TooltipProvider>
      <div className="flex h-svh flex-col overflow-hidden bg-frame">
        <header className="flex min-h-16 shrink-0 flex-wrap items-center justify-between gap-3 px-5 py-2.5 max-[680px]:px-4">
          <div className="flex items-center gap-3">
            <FastShotMark />
            <div className="flex flex-col gap-1">
              <h1 className="font-heading text-base leading-none font-semibold">FastShot</h1>
              <p className="flex items-center gap-1.5 text-xs text-muted-foreground max-[620px]:hidden">
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
          <div className="flex items-center gap-1.5">
            <Button onClick={() => capture("region")} aria-label="Capture a region" title="Capture a region">
              <SquareDashed data-icon="inline-start" />
              <span className="max-[660px]:hidden">Region</span>
            </Button>
            <Button variant="outline" className="bg-background/75 max-[660px]:px-2.5" aria-label="Capture a window" title="Capture a window" onClick={() => capture("window")}>
              <AppWindow data-icon="inline-start" />
              <span className="max-[720px]:hidden">Window</span>
            </Button>
            <Button variant="outline" className="bg-background/75 max-[660px]:px-2.5" aria-label="Capture a display" title="Capture a display" onClick={() => capture("display")}>
              <Monitor data-icon="inline-start" />
              <span className="max-[720px]:hidden">Display</span>
            </Button>
          </div>
        </header>

        {banner ? (
          <div className="shrink-0 px-3 pb-2">
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
          <main className="m-2 mt-0 flex-1 overflow-y-auto rounded-2xl border border-sheet-edge bg-background px-6 py-6">
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
          <Tabs value={tab} onValueChange={setTab} orientation={compactLayout ? "horizontal" : "vertical"} className="min-h-0 flex-1 gap-0 max-[680px]:flex-col">
            <aside className="flex w-48 shrink-0 flex-col justify-between px-3 pb-3 pt-2 max-[680px]:w-full max-[680px]:px-2 max-[680px]:pb-2 max-[680px]:pt-0">
              <TabsList className="h-fit w-full flex-col items-stretch gap-1 bg-transparent p-0 max-[680px]:grid max-[680px]:grid-cols-3 max-[680px]:gap-0">
                {TABS.map(({ value, label, icon: Icon }) => (
                  <TabsTrigger
                    key={value}
                    value={value}
                    className="h-9 flex-none justify-start gap-2.5 rounded-md border-0 px-2.5 text-sm font-normal text-muted-foreground hover:bg-foreground/5 hover:text-foreground focus-visible:border-0 focus-visible:bg-foreground/10 focus-visible:ring-0 focus-visible:outline-none data-active:bg-foreground/5 data-active:font-medium data-active:text-foreground data-active:shadow-none max-[680px]:h-10 max-[680px]:justify-center max-[680px]:rounded-none max-[680px]:px-1 max-[560px]:[&_svg]:hidden [&_svg]:text-muted-foreground data-active:[&_svg]:text-foreground"
                  >
                    <Icon />
                    {label}
                  </TabsTrigger>
                ))}
              </TabsList>
              <div className="flex flex-col gap-1 max-[680px]:hidden">
                <Button variant="ghost" size="sm" className="justify-start text-muted-foreground" onClick={() => void getCurrentWindow().close()}>
                  <X data-icon="inline-start" />
                  Close window
                </Button>
                {info ? <p className="px-2.5 text-[11px] text-muted-foreground/80">Version {info.version}</p> : null}
              </div>
            </aside>

            <main className="m-2 ml-0 mt-0 min-w-0 flex-1 overflow-y-auto rounded-2xl border border-sheet-edge bg-background max-[680px]:m-2 max-[680px]:mt-0">
              <div className="mx-auto flex max-w-2xl flex-col gap-6 px-7 py-7 max-[560px]:px-4 max-[560px]:py-5">
                <div className="flex flex-col gap-1">
                  <h2 className="flex items-center gap-2 font-heading text-xl font-semibold tracking-tight">
                    <current.icon className="size-5 text-capture" />
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
                          <span className="mt-1.5 size-1.5 shrink-0 rounded-full bg-capture" />
                          {t}
                        </li>
                      ))}
                    </ul>
                  </SettingsSection>
                  <SettingsSection icon={Info} title="About">
                    <div className="flex flex-wrap items-center justify-between gap-3">
                      <div className="flex items-center gap-3">
                        <img src="/favicon.svg" alt="" aria-hidden className="size-10 shrink-0" draggable={false} />
                        <div className="flex flex-col gap-0.5 text-sm">
                          <span className="font-medium">FastShot {info?.version}</span>
                          <span className="text-xs text-muted-foreground">Running on {info?.platform}</span>
                        </div>
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
