import { FileDiff } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { PlanPreview } from "@/components/workspace/plan-preview";
import { SnapshotBanner } from "@/components/workspace/snapshot-banner";
import { SourceLedger } from "@/components/workspace/source-ledger";
import { WorkspaceHeader } from "@/components/workspace/workspace-header";
import { useConfirm } from "@/hooks/use-confirm";
import type { ToolAction, ToolScan } from "@/hooks/use-tool-scan";
import * as api from "@/lib/api";
import { getToolMeta } from "@/lib/tools";
import type { ChangePlan } from "@/lib/types";
import { cn } from "@/lib/utils";

interface FlutterPubWorkspaceProps {
  scan: ToolScan;
  operate: (action: ToolAction) => Promise<void>;
  plan: ChangePlan | null;
  setPlan: (plan: ChangePlan | null) => void;
  snapshotId: string | null;
  setSnapshotId: (id: string | null) => void;
  projectDirectory: string;
}

const meta = getToolMeta("flutter-pub");

function profileCardClass(selected: boolean) {
  return cn(
    "rounded-md border p-3 text-left outline-none transition-colors focus-visible:ring-3 focus-visible:ring-ring/30",
    selected
      ? "border-primary bg-primary/5"
      : "border-border bg-card hover:bg-muted",
  );
}

export function FlutterPubWorkspace({
  scan,
  operate,
  plan,
  setPlan,
  snapshotId,
  setSnapshotId,
  projectDirectory,
}: FlutterPubWorkspaceProps) {
  const confirm = useConfirm();
  const [profile, setProfile] = useState<"official" | "custom">("official");
  const [hostedUrl, setHostedUrl] = useState("");

  const loading = scan.status === "loading";

  async function handleScan() {
    await operate(() => api.scanTool(meta, projectDirectory));
  }

  async function previewHosted() {
    await operate(async () => {
      const [result, preview] = await Promise.all([
        api.scanTool(meta, projectDirectory),
        api.previewFlutterPubHostedUpdate({
          hostedUrl: profile === "official" ? null : hostedUrl.trim(),
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
        description:
          "MirrorIt 将创建本地快照后更新用户级 PUB_HOSTED_URL。重新启动 Flutter、终端和 MirrorIt 后才会生效。",
        confirmLabel: "确认应用",
      }))
    ) {
      return;
    }

    await operate(async () => {
      const applied = await api.applyFlutterPubPreview(plan.id);
      const result = await api.scanTool(meta, projectDirectory);
      setSnapshotId(applied.snapshot.id);
      setPlan(null);
      return result;
    });
  }

  async function rollbackSnapshot() {
    if (
      !snapshotId ||
      !(await confirm({
        title: "从快照恢复 PUB_HOSTED_URL？",
        description:
          "当前用户级环境变量将被快照内容替换。重新启动 Flutter、终端和 MirrorIt 后才会生效。",
        confirmLabel: "恢复",
      }))
    ) {
      return;
    }

    await operate(async () => {
      await api.rollbackFlutterPubSnapshot(snapshotId);
      setSnapshotId(null);
      return api.scanTool(meta, projectDirectory);
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

      <section
        aria-labelledby="flutter-pub-edit-heading"
        className="border-b border-border py-6"
      >
        <div className="flex flex-wrap items-end justify-between gap-4">
          <h2 className="text-base font-semibold" id="flutter-pub-edit-heading">
            用户级 hosted 源
          </h2>
          <Button
            disabled={loading || (profile === "custom" && !hostedUrl.trim())}
            onClick={() => void previewHosted()}
            variant="outline"
          >
            <FileDiff aria-hidden="true" />
            查看差异
          </Button>
        </div>
        <div className="mt-4 grid gap-3 lg:grid-cols-2">
          <button
            aria-pressed={profile === "official"}
            className={profileCardClass(profile === "official")}
            onClick={() => setProfile("official")}
            type="button"
          >
            <p className="text-sm font-medium">官方源</p>
            <code className="mt-2 block font-mono text-xs text-muted-foreground">
              https://pub.dev
            </code>
            <p className="mt-2 text-xs text-muted-foreground">
              移除用户级 PUB_HOSTED_URL 覆盖。
            </p>
          </button>
          <button
            aria-pressed={profile === "custom"}
            className={profileCardClass(profile === "custom")}
            onClick={() => setProfile("custom")}
            type="button"
          >
            <p className="text-sm font-medium">自定义源</p>
            <p className="mt-2 text-xs text-muted-foreground">
              设置用户级未含凭据的 HTTPS hosted 源。
            </p>
          </button>
        </div>
        {profile === "custom" ? (
          <label className="mt-4 grid gap-1.5 text-sm font-medium">
            <span>自定义 hosted URL</span>
            <Input
              onChange={(event) => setHostedUrl(event.target.value)}
              placeholder="https://packages.example.com/"
              value={hostedUrl}
            />
          </label>
        ) : null}
        <p className="mt-3 text-xs text-muted-foreground">
          不会修改 pubspec.yaml、锁文件或 .dart_tool。应用后需重新启动
          Flutter、终端和 MirrorIt。
        </p>
      </section>

      {plan ? (
        <PlanPreview
          applying={loading}
          headingId="flutter-pub-plan-heading"
          onApply={() => void applyPreview()}
          plan={plan}
          removedLabel="移除覆盖"
        />
      ) : null}

      {snapshotId ? (
        <SnapshotBanner
          label="已创建 Flutter/Pub 可恢复快照"
          loading={loading}
          onRollback={() => void rollbackSnapshot()}
          snapshotId={snapshotId}
        />
      ) : null}
    </div>
  );
}
