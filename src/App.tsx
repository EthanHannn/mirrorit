import { useState } from "react";
import { Inspector } from "@/components/layout/inspector";
import { Sidebar } from "@/components/layout/sidebar";
import { Titlebar } from "@/components/layout/titlebar";
import { ThemeProvider } from "@/components/theme-provider";
import { FlutterPubWorkspace } from "@/components/tools/flutter-pub-workspace";
import { MavenWorkspace } from "@/components/tools/maven-workspace";
import { NpmWorkspace } from "@/components/tools/npm-workspace";
import { ReadOnlyWorkspace } from "@/components/tools/readonly-workspace";
import { ConfirmProvider } from "@/components/confirm-provider";
import { useToolScans } from "@/hooks/use-tool-scan";
import {
  getToolMeta,
  toolNavigation,
  type ToolId,
  type WritableToolId,
} from "@/lib/tools";
import type { ChangePlan, HealthCheckResult } from "@/lib/types";

const readOnlyTools = toolNavigation.filter((tool) => tool.mode === "只读");

function Shell() {
  const [activeTool, setActiveTool] = useState<ToolId>("npm");
  const [projectDirectory, setProjectDirectory] = useState("");
  const { scans, scan, operate } = useToolScans(projectDirectory);
  const [plans, setPlans] = useState<
    Partial<Record<WritableToolId, ChangePlan | null>>
  >({});
  const [snapshots, setSnapshots] = useState<
    Partial<Record<WritableToolId, string | null>>
  >({});
  const [healthResult, setHealthResult] = useState<HealthCheckResult | null>(
    null,
  );

  const ready = Object.fromEntries(
    toolNavigation.map((tool) => [tool.id, scans[tool.id].result !== null]),
  ) as Record<ToolId, boolean>;

  const setPlan = (tool: WritableToolId) => (plan: ChangePlan | null) =>
    setPlans((current) => ({ ...current, [tool]: plan }));
  const setSnapshot = (tool: WritableToolId) => (id: string | null) =>
    setSnapshots((current) => ({ ...current, [tool]: id }));

  return (
    <div className="grid h-svh grid-rows-[3.25rem_minmax(0,1fr)] overflow-hidden bg-background text-foreground">
      <Titlebar />
      <div className="grid min-h-0 grid-cols-1 grid-rows-[auto_minmax(0,1fr)_auto] overflow-hidden min-[760px]:grid-cols-[13.75rem_minmax(0,1fr)] min-[760px]:grid-rows-[minmax(0,1fr)_auto] min-[1100px]:grid-cols-[13.75rem_minmax(0,1fr)_18rem] min-[1100px]:grid-rows-[minmax(0,1fr)]">
        <Sidebar
          activeTool={activeTool}
          onSelect={setActiveTool}
          ready={ready}
        />
        <main className="min-h-0 min-w-0 overflow-y-auto overscroll-contain bg-card px-4 py-5 min-[760px]:px-6 min-[1100px]:px-8 min-[1100px]:py-7">
          <div className="mx-auto w-full max-w-[70rem]">
            {activeTool === "npm" ? (
              <NpmWorkspace
                healthResult={healthResult}
                operate={(action) => operate("npm", action)}
                plan={plans.npm ?? null}
                projectDirectory={projectDirectory}
                scan={scans.npm}
                setHealthResult={setHealthResult}
                setPlan={setPlan("npm")}
                setProjectDirectory={setProjectDirectory}
                setSnapshotId={setSnapshot("npm")}
                snapshotId={snapshots.npm ?? null}
              />
            ) : null}
            {activeTool === "maven" ? (
              <MavenWorkspace
                operate={(action) => operate("maven", action)}
                plan={plans.maven ?? null}
                scan={scans.maven}
                setPlan={setPlan("maven")}
                setSnapshotId={setSnapshot("maven")}
                snapshotId={snapshots.maven ?? null}
              />
            ) : null}
            {activeTool === "flutter-pub" ? (
              <FlutterPubWorkspace
                operate={(action) => operate("flutter-pub", action)}
                plan={plans["flutter-pub"] ?? null}
                projectDirectory={projectDirectory}
                scan={scans["flutter-pub"]}
                setPlan={setPlan("flutter-pub")}
                setSnapshotId={setSnapshot("flutter-pub")}
                snapshotId={snapshots["flutter-pub"] ?? null}
              />
            ) : null}
            {readOnlyTools.map((tool) =>
              activeTool === tool.id ? (
                <ReadOnlyWorkspace
                  key={tool.id}
                  meta={tool}
                  onScan={() => void scan(tool.id)}
                  scan={scans[tool.id]}
                />
              ) : null,
            )}
          </div>
        </main>
        <Inspector
          healthResult={activeTool === "npm" ? healthResult : null}
          meta={getToolMeta(activeTool)}
          plan={plans[activeTool as WritableToolId] ?? null}
          scan={scans[activeTool]}
          snapshotId={snapshots[activeTool as WritableToolId] ?? null}
        />
      </div>
    </div>
  );
}

function App() {
  return (
    <ThemeProvider>
      <ConfirmProvider>
        <Shell />
      </ConfirmProvider>
    </ThemeProvider>
  );
}

export default App;
