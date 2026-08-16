import { Save } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { ChangePlan } from "@/lib/types";

interface PlanPreviewProps {
  plan: ChangePlan;
  headingId: string;
  applying: boolean;
  onApply: () => void;
  removedLabel?: string;
}

export function PlanPreview({
  plan,
  headingId,
  applying,
  onApply,
  removedLabel = "移除",
}: PlanPreviewProps) {
  return (
    <section
      aria-labelledby={headingId}
      className="border-b border-border py-6"
    >
      <div className="flex flex-wrap items-end justify-between gap-3">
        <h2 className="text-base font-semibold" id={headingId}>
          将要发生的变更
        </h2>
        <div className="flex items-center gap-3">
          <span className="text-xs text-muted-foreground">
            {plan.changes.length} 项字段
          </span>
          <Button disabled={applying} onClick={onApply}>
            <Save aria-hidden="true" />
            确认应用
          </Button>
        </div>
      </div>
      <div className="mt-4 divide-y divide-border border-y border-border">
        {plan.changes.map((change) => (
          <article className="py-4" key={`${change.file}-${change.field}`}>
            <div className="flex flex-wrap items-center justify-between gap-2">
              <p className="font-mono text-sm font-medium">{change.field}</p>
              <p
                className="truncate font-mono text-xs text-muted-foreground"
                title={change.file}
              >
                {change.file}
              </p>
            </div>
            <div className="mt-3 grid gap-2 text-xs md:grid-cols-2">
              <div className="rounded-md bg-muted px-2.5 py-2">
                <p className="mb-1 text-muted-foreground">当前值</p>
                <code className="block truncate font-mono">
                  {change.previous_value ?? "未设置"}
                </code>
              </div>
              <div className="rounded-md bg-primary/5 px-2.5 py-2">
                <p className="mb-1 text-primary">新值</p>
                <code className="block truncate font-mono">
                  {change.next_value ?? removedLabel}
                </code>
              </div>
            </div>
            {change.risk ? (
              <p className="mt-3 border-l border-warning pl-2 text-xs text-warning">
                {change.risk}
              </p>
            ) : null}
          </article>
        ))}
      </div>
    </section>
  );
}
