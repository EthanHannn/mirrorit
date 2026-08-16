import { LoaderCircle, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";

interface WorkspaceHeaderProps {
  title: string;
  description: string;
  scanLabel: string;
  loading: boolean;
  onScan: () => void;
}

export function WorkspaceHeader({
  title,
  description,
  scanLabel,
  loading,
  onScan,
}: WorkspaceHeaderProps) {
  return (
    <section className="flex flex-wrap items-end justify-between gap-4 border-b border-border pb-6">
      <div>
        <h1 className="text-xl font-semibold">{title}</h1>
        <p className="mt-2 max-w-2xl text-sm text-muted-foreground">
          {description}
        </p>
      </div>
      <Button disabled={loading} onClick={onScan}>
        {loading ? (
          <LoaderCircle aria-hidden="true" className="animate-spin" />
        ) : (
          <RefreshCw aria-hidden="true" />
        )}
        {scanLabel}
      </Button>
    </section>
  );
}
