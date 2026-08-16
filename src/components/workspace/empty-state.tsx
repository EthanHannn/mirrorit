import { PackageSearch } from "lucide-react";

export function EmptyState({ toolLabel }: { toolLabel: string }) {
  return (
    <section
      aria-label={`${toolLabel} 未扫描`}
      className="mt-6 flex min-h-28 items-center gap-3 border-y border-hairline py-5"
    >
      <PackageSearch
        aria-hidden="true"
        className="size-5 shrink-0 text-muted-foreground"
      />
      <div>
        <p className="text-sm font-semibold">尚未扫描 {toolLabel}</p>
        <span className="mt-0.5 block text-xs text-muted-foreground">
          扫描后将按优先级展示生效值与来源轨迹。
        </span>
      </div>
    </section>
  );
}
