import { PackageSearch } from "lucide-react";

export function EmptyState({ toolLabel }: { toolLabel: string }) {
  return (
    <section
      aria-label={`${toolLabel} 未扫描`}
      className="my-5 flex min-h-32 items-center gap-5 rounded-lg bg-muted/55 px-5 py-6"
    >
      <PackageSearch
        aria-hidden="true"
        className="size-9 shrink-0 stroke-[1.25] text-primary/70"
      />
      <div>
        <p className="text-sm font-semibold">
          从了解 {toolLabel} 的当前配置开始
        </p>
        <span className="mt-1.5 block text-xs leading-5 text-muted-foreground">
          点击上方扫描，查看生效地址与来源。扫描只读取配置，不会修改文件。
        </span>
      </div>
    </section>
  );
}
