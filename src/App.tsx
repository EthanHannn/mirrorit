import { useState } from "react";
import { FolderSearch, TriangleAlert } from "lucide-react";
import { Input } from "@/components/ui/input";
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

function Shell() {
  const [activeTool, setActiveTool] = useState<ToolId>("npm");
  const [projectDirectory, setProjectDirectory] = useState("");
  const { scans, scan, operate, resetProjectScans } =
    useToolScans(projectDirectory);
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
  const busy = Object.values(scans).some((state) => state.status === "loading");

  function changeProjectDirectory(directory: string) {
    if (busy) return;
    setProjectDirectory(directory);
    resetProjectScans();
    setPlans((current) => ({ ...current, npm: null, "flutter-pub": null }));
  }

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
          scans={scans}
        />
        <main className="flex min-h-0 min-w-0 flex-col bg-card" id="workspace">
          <div className="workspace-context flex flex-wrap items-center gap-x-4 gap-y-2 border-b border-border px-5 py-3 min-[760px]:px-7">
            <label
              className="flex min-w-0 flex-1 items-center gap-2 text-xs font-medium"
              htmlFor="project-directory"
            >
              <FolderSearch
                aria-hidden="true"
                className="size-4 shrink-0 text-muted-foreground"
              />
              <span className="shrink-0">项目目录</span>
              <Input
                id="project-directory"
                className="h-8 min-w-0 text-xs"
                disabled={busy}
                value={projectDirectory}
                onChange={(event) => changeProjectDirectory(event.target.value)}
                placeholder="可选，例如 C:\work\my-project"
              />
            </label>
            <span className="text-xs text-muted-foreground">
              {getToolMeta(activeTool).acceptsProjectDirectory
                ? "留空读取本机配置"
                : "此工具只读取本机配置"}
            </span>
          </div>
          {toolNavigation.map((tool) => (
            <div
              key={tool.id}
              hidden={activeTool !== tool.id}
              className="workspace-panel min-h-0 flex-1 overflow-y-auto overscroll-contain px-5 py-6 min-[760px]:px-7 min-[1100px]:px-8"
            >
              <div className="mx-auto w-full max-w-[70rem]">
                {scans[tool.id].error ? (
                  <div
                    role="alert"
                    className="mb-5 flex items-start gap-3 rounded-md border border-destructive/25 bg-destructive/5 p-3 text-sm"
                  >
                    <TriangleAlert
                      aria-hidden="true"
                      className="mt-0.5 size-4 shrink-0 text-destructive"
                    />
                    <div className="min-w-0">
                      <p className="font-medium">操作未完成</p>
                      <p className="mt-1 break-words text-muted-foreground">
                        {scans[tool.id].error}
                      </p>
                    </div>
                  </div>
                ) : null}
                {tool.id === "npm" ? (
                  <NpmWorkspace
                    healthResult={healthResult}
                    operate={(action) => operate("npm", action)}
                    plan={plans.npm ?? null}
                    projectDirectory={projectDirectory}
                    scan={scans.npm}
                    setHealthResult={setHealthResult}
                    setPlan={setPlan("npm")}
                    setSnapshotId={setSnapshot("npm")}
                    snapshotId={snapshots.npm ?? null}
                  />
                ) : null}
                {tool.id === "maven" ? (
                  <MavenWorkspace
                    operate={(action) => operate("maven", action)}
                    plan={plans.maven ?? null}
                    scan={scans.maven}
                    setPlan={setPlan("maven")}
                    setSnapshotId={setSnapshot("maven")}
                    snapshotId={snapshots.maven ?? null}
                  />
                ) : null}
                {tool.id === "flutter-pub" ? (
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
                {tool.mode === "只读" ? (
                  <ReadOnlyWorkspace
                    key={tool.id}
                    meta={tool}
                    onScan={() => void scan(tool.id)}
                    scan={scans[tool.id]}
                  />
                ) : null}
              </div>
            </div>
          ))}
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
