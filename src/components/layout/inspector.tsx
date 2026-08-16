import {
  CheckCircle2,
  Circle,
  Layers3,
  LoaderCircle,
  LockKeyhole,
  PanelRight,
  TriangleAlert,
} from "lucide-react";
import type { ReactNode } from "react";
import type { ToolScan } from "@/hooks/use-tool-scan";
import type { ToolMeta } from "@/lib/tools";
import type { ChangePlan, HealthCheckResult } from "@/lib/types";

interface InspectorProps {
  meta: ToolMeta;
  scan: ToolScan;
  plan: ChangePlan | null;
  snapshotId: string | null;
  healthResult: HealthCheckResult | null;
}

function Section({ children }: { children: ReactNode }) {
  return (
    <section className="border-hairline p-4 max-[1100px]:min-w-64 max-[1100px]:shrink-0 max-[1100px]:border-r min-[1100px]:border-b">
      {children}
    </section>
  );
}

function SectionHeading({
  title,
  children,
}: {
  title: string;
  children?: ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-2">
      <h3 className="text-[0.8125rem] font-semibold">{title}</h3>
      {children}
    </div>
  );
}

export function Inspector({
  meta,
  scan,
  plan,
  snapshotId,
  healthResult,
}: InspectorProps) {
  const entries = Object.entries(scan.result?.effective_config.values ?? {});
  const sourceCount = entries.reduce(
    (count, [, value]) => count + value.sources.length,
    0,
  );

  return (
    <aside
      aria-label="当前工具检查器"
      className="grid max-h-44 min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)_auto] border-hairline bg-inspector min-[760px]:col-start-2 min-[760px]:max-h-30 min-[760px]:border-t min-[1100px]:col-auto min-[1100px]:max-h-none min-[1100px]:border-t-0 min-[1100px]:border-l"
    >
      <header className="flex min-h-12 items-center gap-2 border-b border-hairline px-4 max-[1100px]:hidden">
        <PanelRight
          aria-hidden="true"
          className="size-3.5 text-muted-foreground"
        />
        <h2 className="text-[0.8125rem] font-semibold">检查器</h2>
      </header>

      <div className="min-h-0 overscroll-contain max-[1100px]:flex max-[1100px]:overflow-x-auto min-[1100px]:overflow-y-auto">
        <Section>
          <div className="flex items-center gap-2.5">
            <span
              aria-hidden="true"
              className="tool-glyph grid size-8 place-items-center rounded-md text-[0.6875rem] font-bold"
              data-tool={meta.id}
            >
              {meta.glyph}
            </span>
            <div>
              <h3 className="text-[0.8125rem] font-semibold">{meta.label}</h3>
              <p className="text-xs text-muted-foreground">{meta.mode}</p>
            </div>
          </div>

          <div
            aria-live="polite"
            className="mt-3.5 mb-1 flex min-w-0 items-start gap-2 text-xs leading-5 text-muted-foreground"
          >
            {scan.status === "loading" ? (
              <>
                <LoaderCircle
                  aria-hidden="true"
                  className="mt-0.5 size-3.5 shrink-0 animate-spin"
                />
                <span>正在读取本机配置…</span>
              </>
            ) : scan.status === "error" ? (
              <>
                <TriangleAlert
                  aria-hidden="true"
                  className="mt-0.5 size-3.5 shrink-0 text-destructive"
                />
                <div className="grid min-w-0 gap-1">
                  <span className="font-semibold text-foreground">
                    读取失败，未修改任何配置。
                  </span>
                  <small className="text-[0.6875rem] text-muted-foreground">
                    请检查工具安装或配置路径后重试。
                  </small>
                  <details>
                    <summary className="cursor-pointer text-[0.6875rem] text-muted-foreground">
                      技术详情
                    </summary>
                    <code className="mt-1 block truncate rounded bg-muted p-1.5 font-mono text-[0.625rem] text-muted-foreground">
                      {scan.error}
                    </code>
                  </details>
                </div>
              </>
            ) : scan.result ? (
              <>
                <CheckCircle2
                  aria-hidden="true"
                  className="mt-0.5 size-3.5 shrink-0 text-success"
                />
                <span>已读取 {entries.length} 项配置</span>
              </>
            ) : (
              <>
                <Circle
                  aria-hidden="true"
                  className="mt-0.5 size-3.5 shrink-0 text-success"
                />
                <span>尚未扫描此工具</span>
              </>
            )}
          </div>
        </Section>

        <Section>
          <SectionHeading title="来源概览">
            <span className="text-xs text-muted-foreground tabular-nums">
              {sourceCount} 条轨迹
            </span>
          </SectionHeading>
          {entries.length ? (
            <ol className="ledger-track mt-3 grid">
              {entries.slice(0, 5).map(([key, value]) => (
                <li
                  className="ledger-node grid min-h-10 grid-cols-[minmax(0,1fr)_auto] items-center gap-1.5 py-1 pl-4"
                  key={key}
                >
                  <div className="min-w-0">
                    <code className="block truncate font-mono text-xs font-semibold">
                      {key}
                    </code>
                    <p
                      className="mt-0.5 truncate text-[0.6875rem] text-muted-foreground"
                      title={value.value ?? "未设置"}
                    >
                      {value.value ?? "未设置"}
                    </p>
                  </div>
                  <span className="min-w-5 text-right text-[0.6875rem] text-muted-foreground tabular-nums">
                    {value.sources.length}
                  </span>
                </li>
              ))}
            </ol>
          ) : (
            <p className="mt-3 text-xs leading-5 text-muted-foreground">
              扫描后，这里会汇总生效值及其来源数量。
            </p>
          )}
          {entries.length > 5 ? (
            <p className="mt-2 text-xs text-muted-foreground">
              另有 {entries.length - 5} 项配置显示在工作区中。
            </p>
          ) : null}
        </Section>

        {plan ? (
          <Section>
            <SectionHeading title="变更预览">
              <span className="text-xs text-muted-foreground tabular-nums">
                {plan.changes.length} 项
              </span>
            </SectionHeading>
            <div className="mt-3 flex gap-2 text-xs leading-5 text-muted-foreground">
              <Layers3
                aria-hidden="true"
                className="size-3.5 shrink-0 text-primary"
              />
              <p>已生成只读差异。确认前不会写入任何配置文件。</p>
            </div>
          </Section>
        ) : null}

        {snapshotId ? (
          <Section>
            <SectionHeading title="恢复点">
              <CheckCircle2
                aria-hidden="true"
                className="size-3.5 text-primary"
              />
            </SectionHeading>
            <code
              className="mt-2.5 block truncate rounded bg-muted p-2 font-mono text-[0.6875rem]"
              title={snapshotId}
            >
              {snapshotId}
            </code>
          </Section>
        ) : null}

        {scan.result?.diagnostics.length ? (
          <Section>
            <SectionHeading title="需要注意">
              <TriangleAlert
                aria-hidden="true"
                className="size-3.5 text-primary"
              />
            </SectionHeading>
            <ul className="mt-3 grid list-disc gap-1.5 pl-4 text-xs leading-5 text-muted-foreground">
              {scan.result.diagnostics.slice(0, 3).map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
          </Section>
        ) : null}

        {healthResult ? (
          <Section>
            <SectionHeading title="连接检查">
              <span className="text-xs text-muted-foreground tabular-nums">
                {healthResult.elapsed_ms} ms
              </span>
            </SectionHeading>
            <p className="mt-3 text-xs leading-5 text-muted-foreground">
              {healthResult.status === "healthy"
                ? "目标地址连接正常。"
                : healthResult.message ||
                  healthResult.status.replace(/_/g, " ")}
            </p>
          </Section>
        ) : null}
      </div>

      <footer className="flex gap-2 border-t border-hairline px-4 py-3 text-[0.6875rem] leading-4.5 text-muted-foreground max-[1100px]:hidden">
        <LockKeyhole
          aria-hidden="true"
          className="mt-0.5 size-3.5 shrink-0 text-success"
        />
        <p>凭据不会出现在导出、日志或来源预览中。</p>
      </footer>
    </aside>
  );
}
