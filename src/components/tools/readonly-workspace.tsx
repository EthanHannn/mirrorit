import { SourceLedger } from "@/components/workspace/source-ledger";
import { WorkspaceHeader } from "@/components/workspace/workspace-header";
import type { ToolScan } from "@/hooks/use-tool-scan";
import type { ToolMeta } from "@/lib/tools";

interface ReadOnlyWorkspaceProps {
  meta: ToolMeta;
  scan: ToolScan;
  onScan: () => void;
}

export function ReadOnlyWorkspace({
  meta,
  scan,
  onScan,
}: ReadOnlyWorkspaceProps) {
  return (
    <div className="pb-8">
      <WorkspaceHeader
        description={meta.description}
        loading={scan.status === "loading"}
        onScan={onScan}
        scanLabel={meta.scanLabel}
        title={meta.title}
      />
      <SourceLedger
        emptyMessage={meta.emptyMessage}
        result={scan.result}
        toolLabel={meta.label}
      />
      {meta.footnote ? (
        <p className="mt-5 border-l border-border px-4 py-3 text-xs text-muted-foreground">
          {meta.footnote}
        </p>
      ) : null}
    </div>
  );
}
