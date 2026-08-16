import { RotateCcw } from "lucide-react";
import { Button } from "@/components/ui/button";

interface SnapshotBannerProps {
  label: string;
  snapshotId: string;
  loading: boolean;
  onRollback: () => void;
}

export function SnapshotBanner({
  label,
  snapshotId,
  loading,
  onRollback,
}: SnapshotBannerProps) {
  return (
    <section className="mt-6 flex flex-wrap items-center justify-between gap-3 border-l-2 border-primary bg-primary/5 px-4 py-3">
      <div className="min-w-0">
        <p className="text-sm font-medium">{label}</p>
        <p
          className="mt-1 truncate font-mono text-xs text-muted-foreground"
          title={snapshotId}
        >
          {snapshotId}
        </p>
      </div>
      <Button disabled={loading} onClick={onRollback} variant="outline">
        <RotateCcw aria-hidden="true" />
        从快照恢复
      </Button>
    </section>
  );
}
