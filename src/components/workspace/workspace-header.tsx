import { LoaderCircle, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";

interface WorkspaceHeaderProps {
  title: string;
  description: string;
  scanLabel: string;
  loading: boolean;
  onScan: () => void;
  hasResult?: boolean;
}

export function WorkspaceHeader({
  title,
  description,
  scanLabel,
  loading,
  onScan,
  hasResult = false,
}: WorkspaceHeaderProps) {
  return (
    <section className="flex flex-wrap items-start justify-between gap-4 border-b border-border pb-5">
      <div className="min-w-0 flex-1 basis-48">
        <h1 className="text-xl font-semibold">{title}</h1>
        <p className="mt-2 max-w-2xl text-sm leading-6 text-muted-foreground">
          {description}
        </p>
      </div>
      <Button disabled={loading} onClick={onScan}>
        {loading ? (
          <LoaderCircle aria-hidden="true" className="animate-spin" />
        ) : (
          <RefreshCw aria-hidden="true" />
        )}
        {loading ? "正在处理…" : hasResult ? "重新扫描" : scanLabel}
      </Button>
    </section>
  );
}
