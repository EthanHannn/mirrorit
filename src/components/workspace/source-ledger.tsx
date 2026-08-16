import { ShieldCheck } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/workspace/empty-state";
import { DiagnosticsSection } from "@/components/workspace/diagnostics-section";
import { scopeLabels } from "@/lib/tools";
import type { ToolReadResult } from "@/lib/types";

interface SourceLedgerProps {
  result: ToolReadResult | null;
  toolLabel: string;
  emptyMessage: string;
}

/**
 * Signature component: for every config key, show the effective value first,
 * then the source track ordered by priority. The winning source gets the
 * filled node and "最终生效" badge.
 */
export function SourceLedger({
  result,
  toolLabel,
  emptyMessage,
}: SourceLedgerProps) {
  if (!result) {
    return <EmptyState toolLabel={toolLabel} />;
  }

  const entries = Object.entries(result.effective_config.values);

  return (
    <section aria-labelledby="ledger-heading" className="pt-6">
      <div className="flex items-baseline justify-between gap-4">
        <h2 id="ledger-heading" className="text-base font-semibold">
          生效配置
        </h2>
        <span className="text-xs text-muted-foreground">
          {entries.length} 项已识别
        </span>
      </div>

      {entries.length ? (
        <div className="mt-4 divide-y divide-border border-y border-border">
          {entries.map(([key, value]) => (
            <article
              className="grid gap-4 py-4 lg:grid-cols-[10rem_minmax(0,1fr)]"
              key={key}
            >
              <div className="min-w-0">
                <p className="truncate font-mono text-sm font-medium">{key}</p>
                <p className="mt-1 text-xs text-muted-foreground">最终生效值</p>
              </div>
              <div className="min-w-0">
                <code
                  className="block truncate rounded-md bg-muted px-2.5 py-2 font-mono text-xs"
                  title={value.value ?? "未设置"}
                >
                  {value.value ?? "未设置"}
                </code>
                <div className="mt-4 flex items-center gap-2">
                  <span className="h-px w-5 bg-border" />
                  <p className="text-xs font-medium text-muted-foreground">
                    来源轨迹
                  </p>
                </div>
                <ol className="ledger-track mt-3 grid gap-2">
                  {value.sources.map((source, index) => {
                    const effective = index === value.sources.length - 1;
                    return (
                      <li
                        className="ledger-node grid gap-1.5 py-1.5 pl-5 text-xs"
                        data-effective={effective}
                        key={`${source.location}-${source.priority}-${index}`}
                      >
                        <div className="flex flex-wrap items-center gap-2">
                          <span className="font-medium">
                            {scopeLabels[source.scope]} · {source.location}
                          </span>
                          <span className="text-muted-foreground tabular-nums">
                            优先级 {source.priority}
                          </span>
                          <Badge variant={effective ? "default" : "outline"}>
                            {effective ? "最终生效" : "被后续来源覆盖"}
                          </Badge>
                          {source.sensitive ? (
                            <span className="inline-flex items-center gap-1 text-warning">
                              <ShieldCheck
                                aria-hidden="true"
                                className="size-3"
                              />
                              凭据已掩盖
                            </span>
                          ) : null}
                        </div>
                        <code
                          className="block truncate font-mono text-muted-foreground"
                          title={source.value ?? "未设置"}
                        >
                          {source.value ?? "未设置"}
                        </code>
                      </li>
                    );
                  })}
                </ol>
              </div>
            </article>
          ))}
        </div>
      ) : (
        <p className="mt-4 border-y border-border py-6 text-sm text-muted-foreground">
          {emptyMessage}
        </p>
      )}

      <DiagnosticsSection diagnostics={result.diagnostics} />
    </section>
  );
}
