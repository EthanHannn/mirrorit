import { FileDiff } from "lucide-react";
import { useState } from "react";
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
import { getToolMeta } from "@/lib/tools";
import type { ChangePlan } from "@/lib/types";

interface MavenWorkspaceProps {
  scan: ToolScan;
  operate: (action: ToolAction) => Promise<void>;
  plan: ChangePlan | null;
  setPlan: (plan: ChangePlan | null) => void;
  snapshotId: string | null;
  setSnapshotId: (id: string | null) => void;
}

const meta = getToolMeta("maven");

export function MavenWorkspace({
  scan,
  operate,
  plan,
  setPlan,
  snapshotId,
  setSnapshotId,
}: MavenWorkspaceProps) {
  const confirm = useConfirm();
  const [mirrorId, setMirrorId] = useState("");
  const [mirrorUrl, setMirrorUrl] = useState("");

  const loading = scan.status === "loading";
  const mirrorEntries = Object.entries(
    scan.result?.effective_config.values ?? {},
  ).filter(
    ([key, value]) =>
      key.startsWith("mirror.") &&
      value.sources.some((source) => source.scope === "user"),
  );

  async function handleScan() {
    await operate(async () => {
      const result = await api.scanTool(meta, "");
      const mirrorIds = Object.entries(result.effective_config.values)
        .filter(
          ([key, value]) =>
            key.startsWith("mirror.") &&
            value.sources.some((source) => source.scope === "user"),
        )
        .map(([key]) => key.slice("mirror.".length));
      setMirrorId((current) =>
        mirrorIds.includes(current) ? current : (mirrorIds[0] ?? ""),
      );
      setPlan(null);
      return result;
    });
  }

  async function previewMirror() {
    await operate(async () => {
      const [result, preview] = await Promise.all([
        api.scanTool(meta, ""),
        api.previewMavenMirrorUpdate({
          mirrorId,
          url: mirrorUrl.trim(),
        }),
      ]);
      setPlan(preview);
      return result;
    });
  }

  async function applyPreview() {
    if (
      !plan ||
      !(await confirm({
        title: "确认应用此预览？",
        description: "MirrorIt 将创建本地快照后更新用户级 Maven settings.xml。",
        confirmLabel: "确认应用",
      }))
    ) {
      return;
    }

    await operate(async () => {
      const applied = await api.applyMavenPreview(plan.id);
      const result = await api.scanTool(meta, "");
      setSnapshotId(applied.snapshot.id);
      setPlan(null);
      return result;
    });
  }

  async function rollbackSnapshot() {
    if (
      !snapshotId ||
      !(await confirm({
        title: "从快照恢复 Maven settings.xml？",
        description: "当前目标文件将被快照内容替换。",
        confirmLabel: "恢复",
      }))
    ) {
      return;
    }

    await operate(async () => {
      await api.rollbackMavenSnapshot(snapshotId);
      setSnapshotId(null);
      return api.scanTool(meta, "");
    });
  }

  return (
    <div className="pb-8">
      <WorkspaceHeader
        description={meta.description}
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

      {mirrorEntries.length ? (
        <section
          aria-labelledby="maven-edit-heading"
          className="border-b border-border py-6"
        >
          <div className="flex flex-wrap items-end justify-between gap-4">
            <h2 className="text-base font-semibold" id="maven-edit-heading">
              更新镜像 URL
            </h2>
            <Button
              disabled={loading || !mirrorId || !mirrorUrl.trim()}
              onClick={() => void previewMirror()}
              variant="outline"
            >
              <FileDiff aria-hidden="true" />
              查看差异
            </Button>
          </div>
          <div className="mt-4 grid gap-3 lg:grid-cols-2">
            <label className="grid gap-1.5 text-sm font-medium">
              <span>目标镜像</span>
              <Select
                onChange={(event) => setMirrorId(event.target.value)}
                value={mirrorId}
              >
                {mirrorEntries.map(([key, value]) => (
                  <option key={key} value={key.slice("mirror.".length)}>
                    {key.slice("mirror.".length)} · {value.value ?? "未设置"}
                  </option>
                ))}
              </Select>
            </label>
            <label className="grid gap-1.5 text-sm font-medium">
              <span>新镜像 URL</span>
              <Input
                onChange={(event) => setMirrorUrl(event.target.value)}
                placeholder="https://repo.example.com/maven/"
                value={mirrorUrl}
              />
            </label>
          </div>
          <p className="mt-3 text-xs text-muted-foreground">
            只接受未含凭据的 HTTPS 地址。预览会复核 XML
            结构和文件校验和，不会写入文件。
          </p>
        </section>
      ) : null}

      {plan ? (
        <PlanPreview
          applying={loading}
          headingId="maven-plan-heading"
          onApply={() => void applyPreview()}
          plan={plan}
        />
      ) : null}

      {snapshotId ? (
        <SnapshotBanner
          label="已创建 Maven 可恢复快照"
          loading={loading}
          onRollback={() => void rollbackSnapshot()}
          snapshotId={snapshotId}
        />
      ) : null}
    </div>
  );
}
