import { Activity, Download, FileDiff } from "lucide-react";
import { useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { PlanPreview } from "@/components/workspace/plan-preview";
import { SnapshotBanner } from "@/components/workspace/snapshot-banner";
import { SourceLedger } from "@/components/workspace/source-ledger";
import { WorkspaceHeader } from "@/components/workspace/workspace-header";
import { useConfirm } from "@/hooks/use-confirm";
import type { ToolAction, ToolScan } from "@/hooks/use-tool-scan";
import * as api from "@/lib/api";
import { applyAndRefresh } from "@/lib/operations";
import { getToolMeta, npmProfileDefinitions } from "@/lib/tools";
import type {
  ChangePlan,
  HealthCheckResult,
  NpmProfileImportPreview,
  NpmProfileSelection,
  ProfileKind,
  TargetScope,
} from "@/lib/types";
import { cn } from "@/lib/utils";

interface NpmWorkspaceProps {
  scan: ToolScan;
  operate: (action: ToolAction) => Promise<void>;
  plan: ChangePlan | null;
  setPlan: (plan: ChangePlan | null) => void;
  snapshotId: string | null;
  setSnapshotId: (id: string | null) => void;
  healthResult: HealthCheckResult | null;
  setHealthResult: (result: HealthCheckResult | null) => void;
  projectDirectory: string;
}

const meta = getToolMeta("npm");

function profileCardClass(selected: boolean) {
  return cn(
    "min-h-24 rounded-md border p-3 text-left outline-none transition-colors focus-visible:ring-3 focus-visible:ring-ring/30",
    selected
      ? "border-primary bg-primary/5"
      : "border-border bg-card hover:bg-muted",
  );
}

export function NpmWorkspace({
  scan,
  operate,
  plan,
  setPlan,
  snapshotId,
  setSnapshotId,
  healthResult,
  setHealthResult,
  projectDirectory,
}: NpmWorkspaceProps) {
  const confirm = useConfirm();
  const [profileKind, setProfileKind] = useState<ProfileKind>("official");
  const [targetScope, setTargetScope] = useState<TargetScope>("user");
  const [customRegistry, setCustomRegistry] = useState("");
  const [healthTarget, setHealthTarget] = useState("");
  const [exportedProfileName, setExportedProfileName] = useState<string | null>(
    null,
  );
  const [importContent, setImportContent] = useState<string | null>(null);
  const [importFileName, setImportFileName] = useState<string | null>(null);
  const [importPreview, setImportPreview] =
    useState<NpmProfileImportPreview | null>(null);
  const [importContext, setImportContext] = useState("");
  const [importError, setImportError] = useState("");
  const fileReadId = useRef(0);

  const loading = scan.status === "loading";

  function selectedProfile(): NpmProfileSelection {
    return profileKind === "custom"
      ? {
          id: "custom-registry",
          name: "自定义源",
          registry: customRegistry.trim(),
        }
      : npmProfileDefinitions[profileKind];
  }

  async function handleScan() {
    await operate(async () => {
      setPlan(null);
      setImportPreview(null);
      return api.scanTool(meta, projectDirectory);
    });
  }

  async function previewProfile() {
    const profile = selectedProfile();
    await operate(async () => {
      setPlan(null);
      const [scanResult, preview] = await Promise.all([
        api.scanTool(meta, projectDirectory),
        api.previewNpmProfile({
          projectDirectory: projectDirectory.trim() || null,
          targetScope,
          profile,
        }),
      ]);
      setPlan(preview);
      return scanResult;
    });
  }

  async function applyPreview() {
    if (
      !plan ||
      !(await confirm({
        title: "确认应用此预览？",
        description: "MirrorIt 将创建本地快照后修改目标 .npmrc。",
        confirmLabel: "确认应用",
      }))
    ) {
      return;
    }

    await operate(async () => {
      return applyAndRefresh(
        () => api.applyNpmPreview(plan.id),
        setSnapshotId,
        () => setPlan(null),
        () => api.scanTool(meta, projectDirectory),
      );
    });
  }

  async function rollbackSnapshot() {
    if (
      !snapshotId ||
      !(await confirm({
        title: "从快照恢复 npm 配置？",
        description: "当前目标文件将被快照内容替换。",
        confirmLabel: "恢复",
      }))
    ) {
      return;
    }

    await operate(async () => {
      await api.rollbackNpmSnapshot(snapshotId);
      setSnapshotId(null);
      setPlan(null);
      return api.scanTool(meta, projectDirectory);
    });
  }

  async function checkHealth() {
    await operate(async () => {
      setHealthResult(null);
      setHealthResult(await api.checkNpmHealth(healthTarget.trim()));
    });
  }

  async function exportProfile() {
    const profile = selectedProfile();
    await operate(async () => {
      const document = await api.exportNpmProfile({ profile });
      const blob = new Blob([`${JSON.stringify(document, null, 2)}\n`], {
        type: "application/json",
      });
      const url = URL.createObjectURL(blob);
      const link = window.document.createElement("a");
      link.href = url;
      link.download = `${document.profiles[0].id}.mirrorit-profile.json`;
      link.click();
      window.setTimeout(() => URL.revokeObjectURL(url), 0);
      setExportedProfileName(document.profiles[0].name);
    });
  }

  async function selectImportFile(file: File | undefined) {
    const readId = ++fileReadId.current;
    setImportPreview(null);
    setImportError("");
    setImportContent(null);
    setImportFileName(null);
    if (!file) {
      return;
    }
    if (file.size > 1024 * 1024) {
      setImportError("配置档过大，请选择不超过 1 MB 的 JSON 文件。");
      return;
    }
    try {
      const content = await file.text();
      if (readId !== fileReadId.current) return;
      setImportContent(content);
      setImportFileName(file.name);
    } catch {
      if (readId === fileReadId.current)
        setImportError("无法读取文件，请重新选择配置档。");
    }
  }

  async function previewImport() {
    if (!importContent) {
      return;
    }
    await operate(async () => {
      setImportPreview(null);
      const current = await api.scanTool(meta, projectDirectory);
      setImportContext(projectDirectory);
      setImportPreview(
        await api.previewNpmProfileImport({
          content: importContent,
          currentRegistry:
            current.effective_config.values.registry?.value ?? "",
        }),
      );
      return current;
    });
  }

  return (
    <fieldset disabled={loading} className="min-w-0 pb-8">
      <WorkspaceHeader
        description={meta.description}
        hasResult={scan.result !== null}
        loading={loading}
        onScan={() => void handleScan()}
        scanLabel={meta.scanLabel}
        title={meta.title}
      />

      <SourceLedger
        emptyMessage={meta.emptyMessage}
        result={scan.result}
        toolLabel={meta.label}
      />

      <section
        aria-labelledby="profile-heading"
        className="border-b border-border py-6"
      >
        <div className="flex flex-wrap items-end justify-between gap-4">
          <h2 className="text-base font-semibold" id="profile-heading">
            预览配置档
          </h2>
          <div className="flex flex-wrap gap-2">
            <Button
              disabled={
                loading || (profileKind === "custom" && !customRegistry.trim())
              }
              onClick={() => void exportProfile()}
              variant="outline"
            >
              <Download aria-hidden="true" />
              导出 JSON
            </Button>
            <Button
              disabled={
                loading ||
                (profileKind === "custom" && !customRegistry.trim()) ||
                (targetScope === "project" && !projectDirectory.trim())
              }
              onClick={() => void previewProfile()}
              variant="outline"
            >
              <FileDiff aria-hidden="true" />
              查看差异
            </Button>
          </div>
        </div>

        <div className="mt-4 grid gap-3 min-[640px]:grid-cols-2 min-[1100px]:grid-cols-3">
          {(
            Object.keys(npmProfileDefinitions) as Array<
              Exclude<ProfileKind, "custom">
            >
          ).map((kind) => {
            const profile = npmProfileDefinitions[kind];
            return (
              <button
                aria-pressed={profileKind === kind}
                className={profileCardClass(profileKind === kind)}
                key={kind}
                onClick={() => {
                  setProfileKind(kind);
                  setPlan(null);
                }}
                type="button"
              >
                <p className="text-sm font-medium">{profile.name}</p>
                <code className="mt-2 block truncate font-mono text-xs text-muted-foreground">
                  {profile.registry}
                </code>
                <p className="mt-2 text-xs leading-4 text-muted-foreground">
                  {profile.description}
                </p>
              </button>
            );
          })}
          <button
            aria-pressed={profileKind === "custom"}
            className={profileCardClass(profileKind === "custom")}
            onClick={() => {
              setProfileKind("custom");
              setPlan(null);
            }}
            type="button"
          >
            <p className="text-sm font-medium">自定义源</p>
            <p className="mt-2 text-xs leading-4 text-muted-foreground">
              使用未含凭据的 HTTPS registry 地址。
            </p>
          </button>
        </div>

        <div className="mt-4 grid gap-3 lg:grid-cols-2">
          {profileKind === "custom" ? (
            <label className="grid gap-1.5 text-sm font-medium">
              <span>自定义 registry</span>
              <Input
                onChange={(event) => {
                  setCustomRegistry(event.target.value);
                  setPlan(null);
                }}
                placeholder="https://registry.example.com/"
                value={customRegistry}
              />
            </label>
          ) : (
            <div className="grid content-end text-sm">
              <p className="text-muted-foreground">
                预览只生成差异，不会修改任何配置文件。
              </p>
            </div>
          )}
          <label className="grid gap-1.5 text-sm font-medium">
            <span>目标作用域</span>
            <Select
              onChange={(event) => {
                setTargetScope(event.target.value as TargetScope);
                setPlan(null);
              }}
              value={targetScope}
            >
              <option value="user">用户级 .npmrc</option>
              <option value="project" disabled={!projectDirectory.trim()}>
                项目级 .npmrc
              </option>
            </Select>
          </label>
        </div>

        {exportedProfileName ? (
          <p className="mt-3 text-xs text-muted-foreground">
            已导出 {exportedProfileName} 的非敏感 JSON 配置档。
          </p>
        ) : null}

        <details className="mt-5 border-t border-border pt-4">
          <summary className="cursor-pointer text-sm font-medium text-muted-foreground">
            从 JSON 导入配置档
          </summary>
          <div className="flex flex-wrap items-end justify-between gap-3">
            <div className="pt-3">
              <p className="mt-1 text-sm text-muted-foreground">
                仅比较所选 JSON 配置档，不会应用任何更改。
              </p>
            </div>
            <Button
              disabled={!importContent || loading}
              onClick={() => void previewImport()}
              variant="outline"
            >
              <FileDiff aria-hidden="true" />
              预览导入
            </Button>
          </div>
          <label className="mt-3 grid gap-1.5 text-sm font-medium">
            <span>导入配置档</span>
            <input
              accept=".json,application/json"
              className="block w-full text-sm text-muted-foreground file:mr-3 file:h-9 file:rounded-md file:border-0 file:bg-muted file:px-3 file:text-sm file:font-medium file:text-foreground hover:file:bg-muted/80"
              onChange={(event) =>
                void selectImportFile(event.target.files?.[0])
              }
              type="file"
            />
          </label>
          {importFileName ? (
            <p className="mt-2 text-xs text-muted-foreground">
              已选择 {importFileName}，尚未应用。
            </p>
          ) : null}
          {importError ? (
            <p role="alert" className="mt-2 text-xs text-destructive">
              {importError}
            </p>
          ) : null}
          {importPreview &&
          importContext === projectDirectory &&
          scan.result ? (
            <div className="mt-3 grid gap-2 border-l border-warning bg-warning/5 px-4 py-3 text-sm">
              <p className="font-medium">
                {importPreview.name}{" "}
                {importPreview.changed
                  ? "与扫描到的 registry 不同"
                  : "与当前 registry 相同"}
              </p>
              <code className="truncate font-mono text-xs text-muted-foreground">
                扫描值：
                {importPreview.current_registry || "当前扫描范围内未设置"}
              </code>
              <code className="truncate font-mono text-xs text-muted-foreground">
                导入：{importPreview.imported_registry}
              </code>
              <p className="text-xs text-muted-foreground">
                此预览未生成写入计划，未修改任何配置。
              </p>
            </div>
          ) : null}
        </details>
      </section>

      {plan ? (
        <PlanPreview
          applying={loading}
          headingId="npm-plan-heading"
          onApply={() => void applyPreview()}
          plan={plan}
        />
      ) : null}

      {snapshotId ? (
        <SnapshotBanner
          label="已创建可恢复快照"
          loading={loading}
          onRollback={() => void rollbackSnapshot()}
          snapshotId={snapshotId}
        />
      ) : null}
      <section
        aria-labelledby="health-heading"
        className="border-b border-border py-6"
      >
        <div className="flex flex-wrap items-end justify-between gap-4">
          <h2 className="text-base font-semibold" id="health-heading">
            npm 源连通性
          </h2>
          <Button
            disabled={loading || !healthTarget.trim()}
            onClick={() => void checkHealth()}
            variant="outline"
          >
            <Activity aria-hidden="true" />
            检查连接
          </Button>
        </div>
        <div className="mt-4 grid gap-3 lg:grid-cols-[1fr_auto]">
          <Input
            aria-label="检查地址"
            onChange={(event) => {
              setHealthTarget(event.target.value);
              setHealthResult(null);
            }}
            placeholder="https://registry.npmjs.org/"
            value={healthTarget}
          />
          {healthResult ? (
            <p className="self-center text-sm text-muted-foreground">
              {healthResult.status === "healthy"
                ? "连接正常"
                : healthResult.status.replace(/_/g, " ")}{" "}
              · {healthResult.elapsed_ms} ms
            </p>
          ) : null}
        </div>
      </section>
    </fieldset>
  );
}
