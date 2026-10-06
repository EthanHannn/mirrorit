import { Search, ShieldCheck } from "lucide-react";
import { useId, useState } from "react";
import { CopyValue } from "@/components/workspace/copy-value";
import { Input } from "@/components/ui/input";
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

// 先展示生效值，再按来源优先级解释覆盖关系。
export function SourceLedger({
  result,
  toolLabel,
  emptyMessage,
}: SourceLedgerProps) {
  const headingId = useId();
  const [query, setQuery] = useState("");
  if (!result) {
    return <EmptyState toolLabel={toolLabel} />;
  }

  const entries = Object.entries(result.effective_config.values);
  const filtered = entries.filter(([key, value]) =>
    `${key} ${value.value ?? ""}`
      .toLowerCase()
      .includes(query.trim().toLowerCase()),
  );

  return (
    <section aria-labelledby={headingId} className="pt-6">
      <div className="flex items-baseline justify-between gap-4">
        <h2 id={headingId} className="text-base font-semibold">
          生效配置
        </h2>
        <span className="text-xs text-muted-foreground">
          {entries.length} 项已识别
        </span>
      </div>

      {entries.length > 3 ? (
        <div className="relative mt-3">
          <Search
            aria-hidden="true"
            className="absolute top-2.5 left-3 size-4 text-muted-foreground"
          />
          <Input
            aria-label="筛选配置项"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="筛选配置名称或地址…"
            className="pl-9"
          />
        </div>
      ) : null}

      {entries.length ? (
        <div className="mt-4 divide-y divide-border border-y border-border">
          {filtered.map(([key, value]) => (
            <article
              className="grid gap-4 py-4 lg:grid-cols-[10rem_minmax(0,1fr)]"
              key={key}
            >
              <div className="min-w-0">
                <p className="break-all font-mono text-sm font-medium">{key}</p>
                <p className="mt-1 text-xs text-muted-foreground">最终生效值</p>
              </div>
              <div className="min-w-0">
                <CopyValue value={value.value ?? "未设置"} />
                <details
                  className="source-details mt-3"
                  open={entries.length <= 3 ? true : undefined}
                >
                  <summary className="cursor-pointer text-xs font-medium text-muted-foreground">
                    来源轨迹 · {value.sources.length} 处来源
                  </summary>
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
                            <span className="min-w-0 break-all font-medium">
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
                </details>
              </div>
            </article>
          ))}
          {filtered.length === 0 ? (
            <p className="py-6 text-sm text-muted-foreground">
              没有匹配的配置项，请尝试其他关键词。
            </p>
          ) : null}
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
